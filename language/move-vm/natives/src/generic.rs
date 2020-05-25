use libra_types::{
    account_address::AccountAddress,
    account_config,
    account_config::{AccountResource, BalanceResource, CORE_CODE_ADDRESS},
    vm_error::{StatusCode, VMStatus},
};
use move_core_types::move_resource::MoveResource;
use move_vm_types::{
    gas_schedule::NativeCostIndex,
    loaded_data::{runtime_types::Type, types::FatType},
    natives::function::{native_gas, NativeContext, NativeResult},
    values::{Struct, Value},
};
use std::collections::VecDeque;
use vm::errors::VMResult;

pub fn native_type_of(
    context: &mut impl NativeContext,
    ty_args: Vec<Type>,
    mut arguments: VecDeque<Value>,
) -> VMResult<NativeResult> {
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
        Err(VMStatus::new(StatusCode::ABORT_TYPE_MISMATCH_ERROR)
            .with_message(format!("expect struct type, but get: {:?}", fat_type)))
    }
}
