use apollo_compiler::{
    values::{OperationDefinition, Selection},
    ApolloCompiler,
};

pub trait OperationDefinitionExt {
    fn max_depth(&self, ctx: &ApolloCompiler) -> usize;
}

impl OperationDefinitionExt for OperationDefinition {
    fn max_depth(&self, ctx: &ApolloCompiler) -> usize {
        return recurse_selections(self.selection_set().selection(), 0, ctx);
    }
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
    use crate::operation_depth::OperationDefinitionExt;

    use apollo_compiler::ApolloCompiler;

    #[test]
    fn basic() {
        let ctx = ApolloCompiler::new(&String::from("{ hello { world } }"));
        let operations = ctx.operations();
        let operation = operations.first().expect("operation missing");
        let depth = operation.max_depth(&ctx);
        assert_eq!(depth, 2);
    }

    #[test]
    fn inline_fragments() {
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
        let ctx = ApolloCompiler::new(op);
        let operations = ctx.operations();
        let operation = operations.first().expect("operation missing");
        let depth = operation.max_depth(&ctx);
        assert_eq!(depth, 4);
    }

    #[test]
    fn named_fragments() {
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
        let ctx = ApolloCompiler::new(op);
        let operations = ctx.operations();
        let operation = operations.first().expect("operation missing");
        let depth = operation.max_depth(&ctx);
        assert_eq!(depth, 4);
    }
}
