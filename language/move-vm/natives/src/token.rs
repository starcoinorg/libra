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
        let mut name = struct_tag.name.as_bytes().to_vec();
        let type_args_info =
            format_type_params(&struct_tag.type_params).expect("format should never fail");
        name.append(&mut type_args_info.into_bytes());
        Ok(NativeResult::ok(
            cost,
            vec![
                Value::address(struct_tag.address),
                Value::vector_u8(struct_tag.module.as_bytes().to_vec()),
                Value::vector_u8(name),
            ],
        ))
    } else {
        //TODO define error code
        Ok(NativeResult::err(cost, DEFAULT_ERROR_CODE))
    }
}

/// Copy from StructTag's display impl.
fn format_type_params(type_params: &[TypeTag]) -> Result<String, std::fmt::Error> {
    use std::fmt::Write;
    let mut f = String::new();
    if let Some(first_ty) = type_params.first() {
        write!(f, "<")?;
        write!(f, "{}", first_ty)?;
        for ty in type_params.iter().skip(1) {
            write!(f, ", {}", ty)?;
        }
        write!(f, ">")?;
    }
    Ok(f)
}

#[test]
fn test_type_params_formatting() {
    use move_core_types::account_address::AccountAddress;
    use move_core_types::identifier::Identifier;
    use move_core_types::language_storage::StructTag;
    let a_struct = StructTag {
        address: AccountAddress::ZERO,
        module: Identifier::new("TestModule").unwrap(),
        name: Identifier::new("TestStruct").unwrap(),
        type_params: vec![TypeTag::Address],
    };
    let cases = vec![
        (vec![TypeTag::Address], "<Address>"),
        (
            vec![TypeTag::Vector(Box::new(TypeTag::U8)), TypeTag::U64],
            "<Vector<U8>, U64>",
        ),
        (
            vec![TypeTag::U64, TypeTag::Struct(a_struct)],
            "<U64, 00000000000000000000000000000000::TestModule::TestStruct<Address>>",
        ),
    ];

    for (ts, expected) in cases {
        let actual = format_type_params(&ts).unwrap();
        assert_eq!(&actual, expected);
    }
}
