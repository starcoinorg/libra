// Copyright (c) The Diem Core Contributors
// SPDX-License-Identifier: Apache-2.0

use crate::{
    data_cache::{RemoteCache, TransactionDataCache},
    interpreter::Interpreter,
    loader::Loader,
    logging::LogContext,
    session::Session,
};
use diem_logger::prelude::*;
use move_core_types::{
    account_address::AccountAddress,
    identifier::IdentStr,
    language_storage::{ModuleId, TypeTag},
    value::{MoveTypeLayout, MoveValue},
    vm_status::StatusCode,
};
use move_vm_types::{
    data_store::DataStore, gas_schedule::CostStrategy, loaded_data::runtime_types::Type,
    values::Value,
};
use vm::{
    access::ModuleAccess,
    compatibility::Compatibility,
    errors::{verification_error, Location, PartialVMError, PartialVMResult, VMResult},
    normalized, CompiledModule, IndexKind,
};

/// An instantiation of the MoveVM.
pub(crate) struct VMRuntime {
    loader: Loader,
}

// signer helper closure
fn is_signer_reference(s: &Type) -> bool {
    match s {
        Type::Reference(ty) => matches!(&**ty, Type::Signer),
        _ => false,
    }
}

fn number_of_signer_ref_params(tys: &[Type]) -> usize {
    for (i, ty) in tys.iter().enumerate() {
        if !is_signer_reference(ty) {
            return i;
        }
    }
    tys.len()
}

impl VMRuntime {
    pub(crate) fn new() -> Self {
        VMRuntime {
            loader: Loader::new(),
        }
    }

    pub fn new_session<'r, R: RemoteCache>(&self, remote: &'r R) -> Session<'r, '_, R> {
        Session {
            runtime: self,
            data_cache: TransactionDataCache::new(remote, &self.loader),
        }
    }

    // See Session::publish_module for what contracts to follow.
    pub(crate) fn publish_module(
        &self,
        module: Vec<u8>,
        _sender: AccountAddress,
        data_store: &mut impl DataStore,
        _cost_strategy: &mut CostStrategy,
        log_context: &impl LogContext,
    ) -> VMResult<()> {
        // deserialize the module. Perform bounds check. After this indexes can be
        // used with the `[]` operator
        let compiled_module = match CompiledModule::deserialize(&module) {
            Ok(module) => module,
            Err(err) => {
                warn!(*log_context, "[VM] module deserialization failed {:?}", err);
                return Err(err.finish(Location::Undefined));
            }
        };

        let module_id = compiled_module.self_id();

        // perform bytecode and loading verification
        self.loader.verify_module_verify_no_missing_dependencies(
            &compiled_module,
            data_store,
            log_context,
        )?;

        data_store.publish_module(&module_id, module)
    }

    fn deserialize_args(&self, tys: &[Type], args: Vec<Vec<u8>>) -> PartialVMResult<Vec<Value>> {
        if tys.len() != args.len() {
            return Err(
                PartialVMError::new(StatusCode::NUMBER_OF_ARGUMENTS_MISMATCH).with_message(
                    format!(
                        "argument length mismatch: expected {} got {}",
                        tys.len(),
                        args.len()
                    ),
                ),
            );
        }

        // Deserialize arguments. This operation will fail if the parameter type is not deserializable.
        //
        // Special rule: `&signer` can be created from data with the layout of `signer`.
        let mut vals = vec![];
        for (ty, arg) in tys.iter().zip(args.into_iter()) {
            let val = if is_signer_reference(ty) {
                match MoveValue::simple_deserialize(&arg, &MoveTypeLayout::Signer) {
                    Ok(MoveValue::Signer(addr)) => {
                        Value::transaction_argument_signer_reference(addr)
                    }
                    Ok(_) | Err(_) => {
                        warn!("[VM] failed to deserialize argument");
                        return Err(PartialVMError::new(
                            StatusCode::FAILED_TO_DESERIALIZE_ARGUMENT,
                        ));
                    }
                }
            } else {
                let layout = match self.loader.type_to_type_layout(ty) {
                    Ok(layout) => layout,
                    Err(_err) => {
                        warn!("[VM] failed to get layout from type");
                        return Err(PartialVMError::new(
                            StatusCode::INVALID_PARAM_TYPE_FOR_DESERIALIZATION,
                        ));
                    }
                };

                match Value::simple_deserialize(&arg, &layout) {
                    Some(val) => val,
                    None => {
                        warn!("[VM] failed to deserialize argument");
                        return Err(PartialVMError::new(
                            StatusCode::FAILED_TO_DESERIALIZE_ARGUMENT,
                        ));
                    }
                }
            };
            vals.push(val)
        }

        Ok(vals)
    }

    fn create_signers_and_arguments(
        &self,
        tys: &[Type],
        senders: Vec<AccountAddress>,
        args: Vec<Vec<u8>>,
    ) -> PartialVMResult<Vec<Value>> {
        // Build the arguments list and check the arguments are of restricted types.
        // Signers are built up from left-to-right. Either all signer arguments are used, or no
        // signer arguments can be be used by a script.
        let n_signer_params = number_of_signer_ref_params(tys);

        let args = if n_signer_params == 0 {
            self.deserialize_args(&tys, args)?
        } else {
            let n_signers = senders.len();
            if n_signer_params != n_signers {
                return Err(
                    PartialVMError::new(StatusCode::NUMBER_OF_SIGNER_ARGUMENTS_MISMATCH)
                        .with_message(format!(
                            "Expected {} signer args got {}",
                            n_signer_params, n_signers
                        )),
                );
            }
            let mut vals: Vec<Value> = senders
                .into_iter()
                .map(Value::transaction_argument_signer_reference)
                .collect();
            vals.extend(self.deserialize_args(&tys[n_signers..], args)?);
            vals
        };

        Ok(args)
    }

    // See Session::execute_script for what contracts to follow.
    pub(crate) fn execute_script(
        &self,
        script: Vec<u8>,
        ty_args: Vec<TypeTag>,
        args: Vec<Vec<u8>>,
        senders: Vec<AccountAddress>,
        data_store: &mut impl DataStore,
        cost_strategy: &mut CostStrategy,
        log_context: &impl LogContext,
    ) -> VMResult<()> {
        // load the script, perform verification
        let (main, ty_args, params) =
            self.loader
                .load_script(&script, &ty_args, data_store, log_context)?;

        let signers_and_args = self
            .create_signers_and_arguments(&params, senders, args)
            .map_err(|err| err.finish(Location::Undefined))?;
        // run the script
        Interpreter::entrypoint(
            main,
            ty_args,
            signers_and_args,
            data_store,
            cost_strategy,
            &self.loader,
            log_context,
        )?;
        Ok(())
    }

    // See Session::execute_script_function for what contracts to follow.
    pub(crate) fn execute_script_function(
        &self,
        module: &ModuleId,
        function_name: &IdentStr,
        ty_args: Vec<TypeTag>,
        args: Vec<Vec<u8>>,
        senders: Vec<AccountAddress>,
        data_store: &mut impl DataStore,
        cost_strategy: &mut CostStrategy,
        log_context: &impl LogContext,
    ) -> VMResult<()> {
        let (func, ty_args, params, _) = self.loader.load_function(
            function_name,
            module,
            &ty_args,
            true, // is_script_execution
            data_store,
            log_context,
        )?;

        let signers_and_args = self
            .create_signers_and_arguments(&params, senders, args)
            .map_err(|err| err.finish(Location::Undefined))?;

        // run the function
        Interpreter::entrypoint(
            func,
            ty_args,
            signers_and_args,
            data_store,
            cost_strategy,
            &self.loader,
            log_context,
        )?;
        Ok(())
    }

    // See Session::execute_function for what contracts to follow.
    pub(crate) fn execute_function(
        &self,
        module: &ModuleId,
        function_name: &IdentStr,
        ty_args: Vec<TypeTag>,
        args: Vec<Vec<u8>>,
        data_store: &mut impl DataStore,
        cost_strategy: &mut CostStrategy,
        log_context: &impl LogContext,
    ) -> VMResult<()> {
        // load the function in the given module, perform verification of the module and
        // its dependencies if the module was not loaded
        let (func, ty_args, params, _) = self.loader.load_function(
            function_name,
            module,
            &ty_args,
            false, // is_script_execution
            data_store,
            log_context,
        )?;

        let params = params
            .into_iter()
            .map(|ty| ty.subst(&ty_args))
            .collect::<PartialVMResult<Vec<_>>>()
            .map_err(|err| err.finish(Location::Undefined))?;

        // check the arguments provided are of restricted types
        let args = self
            .deserialize_args(&params, args)
            .map_err(|err| err.finish(Location::Undefined))?;

        // run the function
        Interpreter::entrypoint(
            func,
            ty_args,
            args,
            data_store,
            cost_strategy,
            &self.loader,
            log_context,
        )?;
        Ok(())
    }

    pub(crate) fn execute_readonly_function(
        &self,
        module: &ModuleId,
        function_name: &IdentStr,
        ty_args: Vec<TypeTag>,
        args: Vec<Vec<u8>>,
        data_store: &mut impl DataStore,
        cost_strategy: &mut CostStrategy,
        log_context: &impl LogContext,
    ) -> VMResult<Vec<(TypeTag, Value)>> {
        // load the function in the given module, perform verification of the module and
        // its dependencies if the module was not loaded
        let (func, ty_args, params, return_type_tags) =
            self.loader
                .load_function(function_name, module, &ty_args, false, data_store, log_context)?;

        let params = params
            .into_iter()
            .map(|ty| ty.subst(&ty_args))
            .collect::<PartialVMResult<Vec<_>>>()
            .map_err(|err| err.finish(Location::Undefined))?;

        // check the arguments provided are of restricted types
        let args = self
            .deserialize_args(&params, args)
            .map_err(|err| err.finish(Location::Undefined))?;

        // run the function
        let returned_value = Interpreter::entrypoint(
            func,
            ty_args,
            args,
            data_store,
            cost_strategy,
            &self.loader,
            log_context,
        )?;

        let result: Vec<_> = return_type_tags
            .into_iter()
            .zip(returned_value.into_iter())
            .collect();
        Ok(result)
    }

    pub(crate) fn loader(&self) -> &Loader {
        &self.loader
    }
}
