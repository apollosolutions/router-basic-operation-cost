use std::{
    collections::HashMap,
    fmt::Display,
    ops::{AddAssign, Deref},
};

use anyhow::{anyhow, Result};
use apollo_compiler::{values::Selection, ApolloCompiler};

use crate::compiler_ext::CompilerAdditions;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Cost(usize);

impl Cost {
    pub fn new(c: usize) -> Self {
        Self(c)
    }
}

impl AddAssign for Cost {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl Display for Cost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

struct Context<'a> {
    compiler: &'a ApolloCompiler,
    cost_map: &'a HashMap<String, usize>,
}

pub fn operation_cost(
    sdl: &str,
    operation: &str,
    operation_name: Option<&str>,
    cost_map: &HashMap<String, usize>,
) -> Result<Cost> {
    let mut input = sdl.to_owned();
    input.push_str(operation);

    let compiler = ApolloCompiler::new(&input);

    let context = Context {
        compiler: &compiler,
        cost_map,
    };

    match compiler.operation_by_name(operation_name) {
        Some(operation) => {
            let parent = compiler
                .operation_root_type(&operation)
                .expect("root type must exist");

            let total_cost = recurse_selections(
                &context,
                operation.selection_set().selection(),
                parent.name(),
            );

            Ok(total_cost)
        }
        None => Err(anyhow!("missing operation")),
    }
}

fn recurse_selections<'a>(
    context: &'a Context,
    selection: &'a [Selection],
    parent_name: &'a str,
) -> Cost {
    let mut cost = Cost(0);

    for selection in selection {
        match selection {
            Selection::Field(f) => {
                if let Some(ty) = f.ty() {
                    let type_name = ty.name();
                    let field_name = f.name();

                    // ignore introspection fields
                    if !type_name.starts_with("__") {
                        let coord = format!("{}.{}", parent_name, field_name);
                        let field_cost = context.cost_map.get(coord.deref()).unwrap_or(&1);

                        tracing::debug!(%coord, %field_cost);

                        cost += Cost(*field_cost);
                        cost +=
                            recurse_selections(context, f.selection_set().selection(), &type_name);
                    }
                } else {
                    tracing::warn!("no type for {}.{}", parent_name, f.name());
                }
            }
            Selection::FragmentSpread(f) => {
                let fragment = f.fragment(&context.compiler.db).expect("qed");

                let parent_name = fragment.type_condition().to_string();
                cost +=
                    recurse_selections(context, fragment.selection_set().selection(), &parent_name);
            }
            Selection::InlineFragment(f) => {
                // ... on ConcreteType
                if let Some(parent_name) = f.type_condition() {
                    cost += recurse_selections(
                        context,
                        f.selection_set().selection(),
                        &String::from(parent_name),
                    );
                // ... @include(if: $x)
                } else {
                    cost += recurse_selections(context, f.selection_set().selection(), parent_name);
                }
            }
        }
    }

    cost
}

#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use anyhow::Result;

    use crate::operation_cost::Cost;

    use super::operation_cost;

    #[test]
    fn basic() -> Result<()> {
        let cost = operation_cost(
            &"type Query { hello: String }".to_string(),
            &"{ hello }".to_string(),
            None,
            &HashMap::from([("Query.hello".to_string(), 10)]),
        )?;
        assert_eq!(cost, Cost(10));
        Ok(())
    }

    #[test]
    fn fragments() -> Result<()> {
        let cost = operation_cost(
            &"type Query { a: A } type A { b: String }".to_string(),
            &"{ a { ...f } } fragment f on A { b }".to_string(),
            None,
            &HashMap::from([("Query.a".to_string(), 5), ("A.b".to_string(), 8)]),
        )?;
        assert_eq!(cost, Cost(13));
        Ok(())
    }

    // Currently fails — cannot find type for field A1.c

    #[test]
    fn abstract_types() -> Result<()> {
        let cost = operation_cost(
            &"type Query { a: A } interface A { b: String } type A1 implements A { b: String c: String }".to_string(),
            &"{ a { b ... on A1 { c } } ".to_string(),
            None,
            &HashMap::from([("Query.a".to_string(), 5), ("A.b".to_string(), 8), ("A1.b".to_string(), 13), ("A1.c".to_string(), 13)]),
        )?;
        assert_eq!(cost, Cost(26));
        Ok(())
    }
}
