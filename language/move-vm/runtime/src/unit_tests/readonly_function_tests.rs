// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{data_cache::RemoteCache, move_vm::MoveVM};
use move_core_types::{
    account_address::AccountAddress,
    gas_schedule::{GasAlgebra, GasUnits},
    identifier::Identifier,
    language_storage::{ModuleId, TypeTag},
};
use move_lang::{compiled_unit::CompiledUnit, shared::Address};
use move_vm_types::gas_schedule::{zero_cost_schedule, CostStrategy};
use move_vm_types::values::*;
use std::{collections::HashMap, path::PathBuf, sync::Arc};
use vm::{
    errors::{PartialVMResult, VMResult},
    CompiledModule,
};

const WORKING_ACCOUNT: AccountAddress =
    AccountAddress::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2]);

struct Adapter {
    store: DataStore,
    vm: Arc<MoveVM>,
}

impl Adapter {
    fn new(store: DataStore) -> Self {
        Self {
            store,
            vm: Arc::new(MoveVM::new()),
        }
    }

    fn publish_modules(&mut self, modules: Vec<CompiledModule>) {
        let mut session = self.vm.new_session(&self.store);
        let cost_table = zero_cost_schedule();
        let mut cost_strategy = CostStrategy::system(&cost_table, GasUnits::new(0));
        for module in modules {
            let mut binary = vec![];
            module
                .serialize(&mut binary)
                .unwrap_or_else(|_| panic!("failure in module serialization: {:#?}", module));
            session
                .publish_module(binary, WORKING_ACCOUNT, &mut cost_strategy)
                .unwrap_or_else(|_| panic!("failure publishing module: {:#?}", module));
        }
        let data = session.finish().expect("failure getting write set");
        for (module_id, module) in data.modules {
            self.store.add_module(module_id, module);
        }
    }

    fn call_readonly_function(&self, module: &ModuleId, name: &Identifier) -> Vec<Value> {
        let cost_table = zero_cost_schedule();
        let mut cost_strategy = CostStrategy::system(&cost_table, GasUnits::new(0));
        let mut session = self.vm.new_session(&self.store);
        session
            .execute_readonly_function(
                module,
                name,
                vec![],
                vec![],
                WORKING_ACCOUNT,
                &mut cost_strategy,
                |e| e,
            )
            .unwrap_or_else(|_| panic!("Failure executing {:?}::{:?}", module, name))
    }
}

#[derive(Clone, Debug)]
struct DataStore {
    modules: HashMap<ModuleId, Vec<u8>>,
}

impl DataStore {
    fn empty() -> Self {
        Self {
            modules: HashMap::new(),
        }
    }

    fn add_module(&mut self, module_id: ModuleId, binary: Vec<u8>) {
        self.modules.insert(module_id, binary);
    }
}

impl RemoteCache for DataStore {
    fn get_module(&self, module_id: &ModuleId) -> VMResult<Option<Vec<u8>>> {
        match self.modules.get(module_id) {
            None => Ok(None),
            Some(binary) => Ok(Some(binary.clone())),
        }
    }

    fn get_resource(
        &self,
        _address: &AccountAddress,
        _tag: &TypeTag,
    ) -> PartialVMResult<Option<Vec<u8>>> {
        Ok(None)
    }
}

fn compile_file(addr: &[u8; AccountAddress::LENGTH]) -> Vec<CompiledModule> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("src/unit_tests/readonly_test.move");
    let s = path.to_str().expect("no path specified").to_owned();
    let (_, modules) =
        move_lang::move_compile(&[s], &[], Some(Address::new(*addr))).expect("Error compiling...");

    let mut compiled_modules = vec![];
    for module in modules {
        match module {
            CompiledUnit::Module { module, .. } => compiled_modules.push(module),
            CompiledUnit::Script { .. } => (),
        }
    }
    compiled_modules
}

#[test]
fn readonly_func_call() -> PartialVMResult<()> {
    let data_store = DataStore::empty();
    let mut adapter = Adapter::new(data_store);
    let modules = compile_file(&[0; 16]);
    adapter.publish_modules(modules);

    let module_id = ModuleId::new(WORKING_ACCOUNT, Identifier::new("A").unwrap());
    let name = Identifier::new("get_1").unwrap();
    let result = adapter.call_readonly_function(&module_id, &name);
    assert!(result[0].equals(&Value::u64(20))?);

    let module_id = ModuleId::new(WORKING_ACCOUNT, Identifier::new("A").unwrap());
    let name = Identifier::new("get_2").unwrap();
    let result = adapter.call_readonly_function(&module_id, &name);
    assert!(result[0].equals(&Value::u64(20))?);
    assert!(result[1].equals(&Value::u64(1))?);

    let module_id = ModuleId::new(WORKING_ACCOUNT, Identifier::new("A").unwrap());
    let name = Identifier::new("get_s").unwrap();
    let result1 = adapter.call_readonly_function(&module_id, &name);
    let value = Value::struct_(Struct::pack(vec![Value::u64(20)], false));
    assert!(result1[0].equals(&value)?);

    Ok(())
}
