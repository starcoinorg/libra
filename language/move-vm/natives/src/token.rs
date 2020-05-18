use move_core_types::language_storage::TypeTag;
use move_vm_types::{
    gas_schedule::NativeCostIndex,
    loaded_data::runtime_types::Type,
    natives::function::{native_gas, NativeContext, NativeResult},
    values::Value,
};
use std::collections::VecDeque;
use vm::errors::PartialVMResult;

const DEFAULT_ERROR_CODE: u64 = 0x0ED2_5519;

/// Return Token types ModuleAddress, ModuleName and StructName
pub fn native_token_name_of(
    context: &mut impl NativeContext,
    ty_args: Vec<Type>,
    arguments: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(arguments.len() == 0);
    //TODO add gas index
    let cost = native_gas(context.cost_table(), NativeCostIndex::LENGTH, 0);
    let type_tag = context.type_to_type_tag(&ty_args[0])?;
    if let TypeTag::Struct(struct_tag) = type_tag {
        Ok(NativeResult::ok(
            cost,
            vec![
                Value::address(struct_tag.address),
                Value::vector_u8(struct_tag.module.as_bytes().to_vec()),
                Value::vector_u8(struct_tag.name.as_bytes().to_vec()),
            ],
        ))
    } else {
        //TODO define error code
        Ok(NativeResult::err(cost, DEFAULT_ERROR_CODE))
    }
}
