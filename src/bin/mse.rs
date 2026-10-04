use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

use mse_core::{Block, BlockId, Checkpoint, CheckpointRelation, Engine, WriteError, BLOCK_SIZE};

fn block(value: u8) -> Block {
    Block::from_bytes([value; BLOCK_SIZE])
}

fn indices(ids: &[BlockId]) -> Vec<usize> {
    ids.iter().copied().map(BlockId::index).collect()
}

fn checkpoint<'a>(
    checkpoints: &'a BTreeMap<u64, Checkpoint>,
    value: &str,
) -> Result<&'a Checkpoint, String> {
    let id = value
        .parse::<u64>()
        .map_err(|_| format!("invalid checkpoint ID: {value}"))?;
    checkpoints
        .get(&id)
        .ok_or_else(|| format!("checkpoint {id} does not exist in this session"))
}

fn engine_mut(engine: &mut Option<Engine>) -> Result<&mut Engine, String> {
    engine
        .as_mut()
        .ok_or_else(|| "create an engine first with: new <blocks>".to_owned())
}

fn engine_ref(engine: &Option<Engine>) -> Result<&Engine, String> {
    engine
        .as_ref()
        .ok_or_else(|| "create an engine first with: new <blocks>".to_owned())
}

fn execute_line(
    line: &str,
    engine: &mut Option<Engine>,
    checkpoints: &mut BTreeMap<u64, Checkpoint>,
) -> Result<Option<String>, String> {
    let parts: Vec<_> = line.split_whitespace().collect();
    if parts.is_empty() {
        return Ok(Some(String::new()));
    }

    match parts.as_slice() {
        [] => Ok(Some(String::new())),
        ["help"] => Ok(Some("commands: new <blocks>, write <index> <byte>, read <index>, checkpoint, checkpoints, inspect [checkpoint-id], diff <checkpoint-id>, restore <checkpoint-id>, discard, commit, help, quit".to_owned())),
        ["new", count] => match count.parse::<usize>() {
            Ok(count) => {
                *engine = Some(Engine::new(count));
                checkpoints.clear();
                Ok(Some(format!("created: blocks={count}")))
            }
            Err(_) => Err(format!("invalid block count: {count}")),
        },
        ["write", index, byte] => {
            let index = index
                .parse::<usize>()
                .map_err(|_| format!("invalid block index: {index}"));
            let byte = byte
                .parse::<u8>()
                .map_err(|_| format!("invalid byte value: {byte}"));
            match (index, byte) {
                (Ok(index), Ok(byte)) => match engine_mut(engine)?.write(BlockId::new(index), block(byte)) {
                    Ok(()) => Ok(Some(format!("written: block={index} value={byte}"))),
                    Err(WriteError::OutOfRange { block_count, .. }) => {
                        Err(format!("block {index} is out of range (blocks={block_count})"))
                    }
                },
                (Err(error), _) | (_, Err(error)) => Err(error),
            }
        }
        ["read", index] => match index.parse::<usize>() {
            Ok(index) => match engine_ref(engine)?.read(BlockId::new(index)) {
                Some(block) => Ok(Some(format!("read: block={index} value={}", block.as_bytes()[0]))),
                None => Err(format!("block {index} is out of range")),
            },
            Err(_) => Err(format!("invalid block index: {index}")),
        },
        ["checkpoint"] => {
            let saved = engine_ref(engine)?.checkpoint();
            let id = saved.id().value();
            checkpoints.insert(id, saved);
            Ok(Some(format!("checkpoint: id={id}")))
        }
        ["checkpoints"] => {
            let ids: Vec<_> = checkpoints.keys().copied().collect();
            Ok(Some(format!("checkpoints: {ids:?}")))
        }
        ["inspect"] | ["inspect", _] => {
            let saved = match parts.get(1) {
                Some(id) => Some(checkpoint(checkpoints, id)?),
                None => None,
            };
            let state = engine_ref(engine)?.inspect(saved);
            let mut lines = vec![format!(
                "state: blocks={} dirty={} changes={} affected={:?}",
                state.block_count,
                state.dirty_count,
                state.has_changes,
                state.affected_blocks.iter().map(|id| id.index()).collect::<Vec<_>>()
            )];
            lines.push(match state.checkpoint {
                CheckpointRelation::NotCompared => "checkpoint: not-compared".to_owned(),
                CheckpointRelation::Compatible { checkpoint_id, changed_blocks } => format!(
                    "checkpoint: id={} compatible changed={:?}",
                    checkpoint_id.value(),
                    indices(&changed_blocks)
                ),
                CheckpointRelation::Incompatible { checkpoint_id, checkpoint_block_count } => format!(
                    "checkpoint: id={} incompatible blocks={checkpoint_block_count}",
                    checkpoint_id.value()
                ),
            });
            Ok(Some(lines.join("\n")))
        }
        ["diff", id] => {
            let saved = checkpoint(checkpoints, id)?;
            match engine_ref(engine)?.diff(saved) {
                Ok(changed) => {
                    Ok(Some(format!("diff: blocks={:?}", indices(&changed))))
                }
                Err(_) => Err(format!("checkpoint {id} is incompatible with this engine")),
            }
        }
        ["restore", id] => {
            let saved = checkpoint(checkpoints, id)?.clone();
            match engine_mut(engine)?.restore(&saved) {
                Ok(()) => {
                    Ok(Some(format!("restored: checkpoint={id}")))
                }
                Err(_) => Err(format!("checkpoint {id} is incompatible with this engine")),
            }
        }
        ["discard"] => {
            engine_mut(engine)?.discard();
            Ok(Some("discarded".to_owned()))
        }
        ["commit"] => {
            engine_mut(engine)?.commit();
            Ok(Some("committed".to_owned()))
        }
        ["quit"] | ["exit"] => Ok(None),
        [command, ..] => Err(format!("unknown command or invalid arguments: {command}")),
    }
}

fn run(input: impl BufRead, mut output: impl Write) -> io::Result<()> {
    let mut engine = None;
    let mut checkpoints = BTreeMap::new();
    for line in input.lines() {
        let line = line?;
        match execute_line(&line, &mut engine, &mut checkpoints) {
            Ok(Some(response)) if !response.is_empty() => writeln!(output, "{response}")?,
            Ok(Some(_)) => {}
            Ok(None) => break,
            Err(error) => writeln!(output, "error: {error}")?,
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    run(stdin.lock(), stdout.lock())
}

#[cfg(test)]
mod tests {
    use super::run;
    use std::io::Cursor;

    #[test]
    fn session_runs_product_lifecycle_commands() {
        let input = concat!(
            "new 2\n",
            "write 0 12\n",
            "checkpoint\n",
            "write 1 34\n",
            "diff 1\n",
            "restore 1\n",
            "read 0\n",
            "read 1\n",
            "commit\n",
            "write 1 56\n",
            "discard\n",
            "inspect\n",
            "quit\n"
        );
        let mut output = Vec::new();
        run(Cursor::new(input), &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("created: blocks=2"));
        assert!(output.contains("diff: blocks=[1]"));
        assert!(output.contains("read: block=0 value=12"));
        assert!(output.contains("read: block=1 value=0"));
        assert!(output.contains("state: blocks=2 dirty=0 changes=false affected=[]"));
    }

    #[test]
    fn invalid_commands_are_reported_and_session_continues() {
        let input = "read 0\nnew 1\nwrite 9 2\nread 0\nquit\n";
        let mut output = Vec::new();
        run(Cursor::new(input), &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("error: create an engine first"));
        assert!(output.contains("error: block 9 is out of range (blocks=1)"));
        assert!(output.contains("read: block=0 value=0"));
    }
}
