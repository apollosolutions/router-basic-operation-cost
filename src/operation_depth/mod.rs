use anyhow::Result;
use apollo_compiler::{values::Selection, ApolloCompiler};

use crate::compiler_ext::CompilerAdditions;

pub fn operation_depth(operation: &String, operation_name: Option<&String>) -> Result<usize> {
    let compiler = ApolloCompiler::new(&operation);

    if let Some(operation) = compiler.operation_by_name(operation_name) {
        let depth = recurse_selections(operation.selection_set().selection(), 0, &compiler);
        return Ok(depth);
    }

    Err(anyhow::format_err!("missing operation"))
}

fn recurse_selections(selections: &[Selection], depth: usize, ctx: &ApolloCompiler) -> usize {
    let mut max_depth = depth;

    for selection in selections {
        match selection {
            Selection::Field(f) => {
                let new_depth = recurse_selections(f.selection_set().selection(), depth + 1, ctx);
                if new_depth > max_depth {
                    max_depth = new_depth
                }
            }
            Selection::FragmentSpread(f) => {
                if let Some(fragment) = f.fragment(&ctx.db) {
                    let new_depth =
                        recurse_selections(fragment.selection_set().selection(), depth + 1, ctx);
                    if new_depth > max_depth {
                        max_depth = new_depth
                    }
                }
            }
            Selection::InlineFragment(f) => {
                let new_depth = recurse_selections(f.selection_set().selection(), depth + 1, ctx);
                if new_depth > max_depth {
                    max_depth = new_depth
                }
            }
        }
    }

    max_depth
}

#[cfg(test)]
mod tests {
    use super::operation_depth;

    use anyhow::Result;

    #[test]
    fn basic() -> Result<()> {
        let depth = operation_depth(&String::from("{ hello { world } }"), None)?;
        assert_eq!(depth, 2);
        Ok(())
    }

    #[test]
    fn inline_fragments() -> Result<()> {
        let op = &String::from(
            "
{
  a {
    ... on B {
      c
      d {
        e
      }
    }
  }
}",
        );
        let depth = operation_depth(op, None)?;
        assert_eq!(depth, 4);
        Ok(())
    }

    #[test]
    fn named_fragments() -> Result<()> {
        let op = &String::from(
            "
fragment f on B {
  c
  d {
    e
  }
}

{
  a {
    ...f
  }
}",
        );
        let depth = operation_depth(op, None)?;
        assert_eq!(depth, 4);
        Ok(())
    }
}
