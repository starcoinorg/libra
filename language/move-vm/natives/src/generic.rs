use move_vm_types::loaded_data::types::FatType;
use move_vm_types::{
    gas_schedule::NativeCostIndex,
    loaded_data::runtime_types::Type,
    natives::function::{native_gas, NativeContext, NativeResult},
    values::Value,
};
use std::collections::VecDeque;
use vm::errors::PartialVMResult;

const DEFAULT_ERROR_CODE: u64 = 0x0ED2_5519;

pub fn native_type_of(
    context: &mut impl NativeContext,
    ty_args: Vec<Type>,
    arguments: VecDeque<Value>,
) -> PartialVMResult<NativeResult> {
    debug_assert!(ty_args.len() == 1);
    debug_assert!(arguments.len() == 0);
    //TODO add gas index
    let cost = native_gas(context.cost_table(), NativeCostIndex::LENGTH, 0);
    let fat_type = context
        .convert_to_fat_types(ty_args)?
        .pop()
        .expect("Must have at least one type");
    if let FatType::Struct(fat_struct_type) = fat_type {
        Ok(NativeResult::ok(
            cost,
            vec![
                Value::address(fat_struct_type.address),
                Value::vector_u8(fat_struct_type.module.as_bytes().to_vec()),
                Value::vector_u8(fat_struct_type.name.as_bytes().to_vec()),
            ],
        ))
    } else {
        //TODO define error code
        Ok(NativeResult::err(cost, DEFAULT_ERROR_CODE))
    }
}
