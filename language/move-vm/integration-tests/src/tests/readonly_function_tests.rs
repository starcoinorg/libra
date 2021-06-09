// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::{errors::PartialVMResult, CompiledModule};
use move_core_types::language_storage::StructTag;
use move_core_types::{
    account_address::AccountAddress,
    identifier::Identifier,
    language_storage::{ModuleId, TypeTag},
};
use move_lang::compiled_unit::CompiledUnit;
use move_lang::shared::Flags;
use move_vm_runtime::move_vm_adapter::MoveVMAdapter;
use move_vm_test_utils::InMemoryStorage;
use move_vm_types::gas_schedule::GasStatus;
use move_vm_types::values::*;
use std::{path::PathBuf, sync::Arc};

const WORKING_ACCOUNT: AccountAddress =
    AccountAddress::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);

struct Adapter {
    store: InMemoryStorage,
    vm: Arc<MoveVMAdapter>,
}

impl Adapter {
    fn new(store: InMemoryStorage) -> Self {
        Self {
            store,
            vm: Arc::new(MoveVMAdapter::new()),
        }
    }

    fn publish_modules(&mut self, modules: Vec<CompiledModule>) {
        let mut session = self.vm.new_session(&self.store);
        let mut gas_status = GasStatus::new_unmetered();
        for module in modules {
            let mut binary = vec![];
            module
                .serialize(&mut binary)
                .unwrap_or_else(|_| panic!("failure in module serialization: {:#?}", module));
            session
                .publish_module(binary, WORKING_ACCOUNT, &mut gas_status)
                .unwrap_or_else(|_| panic!("failure publishing module: {:#?}", module));
        }
        let (changeset, _) = session.finish().expect("failure getting write set");
        self.store
            .apply(changeset)
            .expect("failure applying write set");
    }

    fn call_readonly_function(
        &self,
        module: &ModuleId,
        name: &Identifier,
    ) -> Vec<(TypeTag, Value)> {
        let mut gas_status = GasStatus::new_unmetered();
        let mut session = self.vm.new_session(&self.store);
        session
            .execute_readonly_function(module, name, vec![], vec![], &mut gas_status)
            .unwrap_or_else(|_| panic!("Failure executing {:?}::{:?}", module, name))
    }
}

fn compile_file() -> Vec<CompiledModule> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/tests/readonly_test.move");
    let s = path.to_str().expect("no path specified").to_owned();
    let (_, modules) = move_lang::move_compile(&[s], &[], None, Flags::empty())
        .expect("Compiling test module failed.");

    let mut compiled_modules = vec![];
    for module in modules.expect("Unwrapping CompiledUnit failed.") {
        match module {
            CompiledUnit::Module { module, .. } => compiled_modules.push(module),
            CompiledUnit::Script { .. } => (),
        }
    }
    compiled_modules
}

#[test]
fn readonly_func_call() -> PartialVMResult<()> {
    let data_store = InMemoryStorage::new();
    let mut adapter = Adapter::new(data_store);
    let modules = compile_file();
    adapter.publish_modules(modules);

    let module_id = ModuleId::new(WORKING_ACCOUNT, Identifier::new("A").unwrap());
    let name = Identifier::new("get_1").unwrap();
    let result = adapter.call_readonly_function(&module_id, &name);
    assert_eq!(result[0].0, TypeTag::U64);
    assert!(result[0].1.equals(&Value::u64(20))?);

    let module_id = ModuleId::new(WORKING_ACCOUNT, Identifier::new("A").unwrap());
    let name = Identifier::new("get_2").unwrap();
    let result = adapter.call_readonly_function(&module_id, &name);
    assert_eq!(result[0].0, TypeTag::U64);
    assert!(result[0].1.equals(&Value::u64(20))?);
    assert_eq!(result[1].0, TypeTag::U64);
    assert!(result[1].1.equals(&Value::u64(1))?);

    let module_id = ModuleId::new(WORKING_ACCOUNT, Identifier::new("A").unwrap());
    let name = Identifier::new("get_s").unwrap();
    let result = adapter.call_readonly_function(&module_id, &name);
    let value = Value::struct_(Struct::pack(vec![Value::u64(20)]));
    let return_type_tag = TypeTag::Struct(StructTag {
        address: AccountAddress::from_hex_literal("0x2").unwrap(),
        module: Identifier::new("A").unwrap(),
        name: Identifier::new("S").unwrap(),
        type_params: vec![],
    });
    assert_eq!(result[0].0, return_type_tag);
    assert!(result[0].1.equals(&value)?);

    Ok(())
}
