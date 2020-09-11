// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    data_cache::{RemoteCache, TransactionDataCache, TransactionEffects},
    runtime::VMRuntime,
};
use libra_logger::prelude::*;
use move_core_types::{
    account_address::AccountAddress,
    identifier::IdentStr,
    language_storage::{ModuleId, TypeTag},
    vm_status::VMStatus,
};
use move_vm_types::data_store::DataStore;
use move_vm_types::{gas_schedule::CostStrategy, values::Value};
use vm::errors::*;
use vm::CompiledModule;

pub struct Session<'r, 'l, R> {
    pub(crate) runtime: &'l VMRuntime,
    pub(crate) data_cache: TransactionDataCache<'r, 'l, R>,
}

impl<'r, 'l, R: RemoteCache> Session<'r, 'l, R> {
    pub fn execute_function<F: FnOnce(VMStatus) -> VMStatus>(
        &mut self,
        module: &ModuleId,
        function_name: &IdentStr,
        ty_args: Vec<TypeTag>,
        args: Vec<Value>,
        _sender: AccountAddress,
        cost_strategy: &mut CostStrategy,
        error_specializer: F,
    ) -> Result<(), VMStatus> {
        self.runtime
            .execute_function(
                module,
                function_name,
                ty_args,
                args,
                &mut self.data_cache,
                cost_strategy,
            )
            .map_err(|e| error_specializer(e.into_vm_status()))
    }

    pub fn execute_readonly_function<F: FnOnce(VMStatus) -> VMStatus>(
        &mut self,
        module: &ModuleId,
        function_name: &IdentStr,
        ty_args: Vec<TypeTag>,
        args: Vec<Value>,
        cost_strategy: &mut CostStrategy,
        error_specializer: F,
    ) -> Result<Vec<(TypeTag, Value)>, VMStatus> {
        self.runtime
            .execute_readonly_function(
                module,
                function_name,
                ty_args,
                args,
                &mut self.data_cache,
                cost_strategy,
            )
            .map_err(|e| error_specializer(e.into_vm_status()))
    }

    pub fn execute_script(
        &mut self,
        script: Vec<u8>,
        ty_args: Vec<TypeTag>,
        args: Vec<Value>,
        senders: Vec<AccountAddress>,
        cost_strategy: &mut CostStrategy,
    ) -> VMResult<()> {
        self.runtime.execute_script(
            script,
            ty_args,
            args,
            senders,
            &mut self.data_cache,
            cost_strategy,
        )
    }

    pub fn publish_module(
        &mut self,
        module: Vec<u8>,
        sender: AccountAddress,
        cost_strategy: &mut CostStrategy,
    ) -> VMResult<()> {
        self.runtime
            .publish_module(module, sender, &mut self.data_cache, cost_strategy)
    }

    pub fn verify_module(&mut self, module: &[u8]) -> VMResult<CompiledModule> {
        let compiled_module = match CompiledModule::deserialize(module) {
            Ok(module) => module,
            Err(err) => {
                warn!("[VM] module deserialization failed {:?}", err);
                return Err(err.finish(Location::Undefined));
            }
        };
        self.runtime
            .loader()
            .verify_module_verify_no_missing_dependencies(&compiled_module, &mut self.data_cache)?;
        Ok(compiled_module)
    }

    pub fn exists_module(&self, module_id: &ModuleId) -> VMResult<bool> {
        self.data_cache.exists_module(module_id)
    }

    pub fn load_module(&self, module_id: &ModuleId) -> VMResult<Vec<u8>> {
        self.data_cache.load_module(module_id)
    }

    pub fn num_mutated_accounts(&self) -> u64 {
        self.data_cache.num_mutated_accounts()
    }

    pub fn finish(self) -> VMResult<TransactionEffects> {
        self.data_cache
            .into_effects()
            .map_err(|e| e.finish(Location::Undefined))
    }
}
