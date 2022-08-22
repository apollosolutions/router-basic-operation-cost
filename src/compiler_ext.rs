use apollo_compiler::{
    values::{ObjectTypeDefinition, OperationDefinition},
    ApolloCompiler,
};

pub trait CompilerAdditions {
    fn operation_by_name(&self, operation_name: Option<&str>) -> Option<OperationDefinition>;
    fn operation_root_type(&self, operation: &OperationDefinition) -> Option<ObjectTypeDefinition>;
}

impl CompilerAdditions for ApolloCompiler {
    fn operation_by_name(&self, operation_name: Option<&str>) -> Option<OperationDefinition> {
        if let Some(op_name) = operation_name {
            if let Some(operation) = self
                .operations()
                .iter()
                .find(|op| op.name().unwrap_or_default().eq(op_name))
            {
                return Some(operation.clone());
            }
        } else if self.operations().len() == 1 {
            return Some(self.operations().first().expect("qed").clone());
        }

        None
    }

    fn operation_root_type(&self, operation: &OperationDefinition) -> Option<ObjectTypeDefinition> {
        self.object_types()
            .iter()
            .find(|ty| ty.name() == operation.operation_ty().to_string())
            .cloned()
    }
}
