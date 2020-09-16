// Copyright (c) The Libra Core Contributors
// SPDX-License-Identifier: Apache-2.0

#[allow(unused_imports)]
use log::{debug, info, warn};

use crate::module_abi::{
    FuncArgumentABI, FuncDefinitionABI, ModuleABI, ReferenceABI, StructABI, StructDefinitionABI,
    StructFieldABI, TypeABI,
};
use anyhow::bail;
use heck::SnakeCase;
use libra_types::transaction::{ArgumentABI, ScriptABI, TypeArgumentABI};
use move_core_types::language_storage::TypeTag;
use serde::{Deserialize, Serialize};
use spec_lang::ty::PrimitiveType;
use spec_lang::{
    env::{GlobalEnv, ModuleEnv},
    ty,
};
use std::{collections::BTreeMap, io::Read, path::PathBuf};

/// Options passed into the ABI generator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AbigenOptions {
    /// Where to find the .mv files of scripts.
    pub compiled_script_directory: String,
    /// In which directory to store output.
    pub output_directory: String,
    /// whether gen abi for dependencies.
    pub gen_deps: bool,
}

impl Default for AbigenOptions {
    fn default() -> Self {
        Self {
            compiled_script_directory: ".".to_string(),
            output_directory: "abi".to_string(),
            gen_deps: false,
        }
    }
}

/// The ABI generator.
pub struct Abigen<'env> {
    /// Options.
    options: &'env AbigenOptions,
    /// Input definitions.
    env: &'env GlobalEnv,
    /// Map from file name to generated script ABI (if any).
    output: BTreeMap<String, ScriptABI>,
    module_output: BTreeMap<String, ModuleABI>,
}

impl<'env> Abigen<'env> {
    /// Creates a new ABI generator.
    pub fn new(env: &'env GlobalEnv, options: &'env AbigenOptions) -> Self {
        Self {
            options,
            env,
            output: Default::default(),
            module_output: Default::default(),
        }
    }

    /// Returns the result of ABI generation, a vector of pairs of filenames
    /// and JSON content.
    pub fn into_result(mut self) -> Vec<(String, Vec<u8>)> {
        let modules = std::mem::take(&mut self.module_output)
            .into_iter()
            .map(|(path, abi)| {
                let content = serde_json::to_vec_pretty(&abi).expect("ABI to toml should not fail");
                // let content = lcs::to_bytes(&abi).expect("ABI serialization should not fail");
                (path, content)
            });
        std::mem::take(&mut self.output)
            .into_iter()
            .map(|(path, abi)| {
                let content = lcs::to_bytes(&abi).expect("ABI serialization should not fail");
                (path, content)
            })
            .chain(modules)
            .collect()
    }

    /// Generates ABIs for all script modules in the environment (excluding the dependency set).
    pub fn gen(&mut self) {
        for module in self.env.get_modules() {
            if module.is_dependency() && !self.options.gen_deps {
                continue;
            }
            let mut path = PathBuf::from(&self.options.output_directory);
            path.push(
                PathBuf::from(module.get_source_path())
                    .with_extension("abi")
                    .file_name()
                    .expect("file name"),
            );

            if module.is_script_module() {
                match self.compute_abi(&module) {
                    Ok(abi) => {
                        self.output.insert(path.to_string_lossy().to_string(), abi);
                    }
                    Err(error) => panic!(
                        "Error while processing script file {:?}: {}",
                        module.get_source_path(),
                        error
                    ),
                }
            } else {
                match self.compute_module_abi(&module) {
                    Ok(abi) => {
                        self.module_output
                            .insert(path.to_string_lossy().to_string(), abi);
                    }
                    Err(error) => panic!(
                        "Error while processing module file {:?}: {}",
                        module.get_source_path(),
                        error
                    ),
                }
            }
        }
    }

    /// Compute the ABI of a script module.
    fn compute_abi(&self, module_env: &ModuleEnv<'env>) -> anyhow::Result<ScriptABI> {
        let symbol_pool = module_env.symbol_pool();
        let func = match module_env.get_functions().next() {
            Some(f) => f,
            None => bail!("A script module should define a function."),
        };
        let name = symbol_pool.string(func.get_name()).to_string();
        let doc = func.get_doc().to_string();
        let code = self.load_compiled_bytes(&module_env)?.to_vec();
        let ty_args = func
            .get_named_type_parameters()
            .iter()
            .map(|ty_param| {
                TypeArgumentABI::new(symbol_pool.string(ty_param.0).to_string().to_snake_case())
            })
            .collect();
        let args = func
            .get_parameters()
            .iter()
            .filter_map(
                |param| match self.get_type_tag_skipping_references(&param.1) {
                    Ok(Some(tag)) => Some(Ok(ArgumentABI::new(
                        symbol_pool.string(param.0).to_string(),
                        tag,
                    ))),
                    Ok(None) => None,
                    Err(error) => Some(Err(error)),
                },
            )
            .collect::<anyhow::Result<_>>()?;
        Ok(ScriptABI::new(name, doc, code, ty_args, args))
    }

    fn type_to_type_abi(
        &self,
        ty0: ty::Type,
        ty_args: &[TypeArgumentABI],
    ) -> anyhow::Result<TypeABI> {
        use ty::Type;
        let abi = match ty0 {
            Type::Primitive(p) => match p {
                ty::PrimitiveType::U8 => TypeABI::U8,
                PrimitiveType::Bool => TypeABI::Bool,
                PrimitiveType::U64 => TypeABI::U64,
                PrimitiveType::U128 => TypeABI::U128,
                PrimitiveType::Address => TypeABI::Address,
                PrimitiveType::Signer => TypeABI::Signer,
                _ => {
                    bail!("cannot gen type api for {}", p);
                }
            },
            Type::Tuple(t) => {
                let mut ts = vec![];
                for ty in t {
                    let type_abi = self.type_to_type_abi(ty, ty_args)?;
                    ts.push(type_abi);
                }
                TypeABI::Tuple(ts)
            }
            Type::Vector(inner_type) => {
                let t = self.type_to_type_abi(*inner_type, ty_args)?;
                TypeABI::Vector(Box::new(t))
            }
            Type::Struct(mid, sid, tys) => {
                let module_env = self.env.get_module(mid);
                let symbol_pool = module_env.symbol_pool();
                let struct_env = module_env.get_struct(sid);
                let mut ty_params = vec![];
                for t in tys {
                    let t = self.type_to_type_abi(t, ty_args)?;
                    ty_params.push(t);
                }
                let abi = StructABI {
                    address: *module_env.self_address(),
                    module: symbol_pool.string(module_env.get_name().name()).to_string(),
                    name: symbol_pool.string(struct_env.get_name()).to_string(),
                    ty_args: ty_params,
                };
                TypeABI::Struct(Box::new(abi))
            }
            Type::TypeParameter(idx) => {
                TypeABI::TypeArgument(Box::new(ty_args[idx as usize].clone()))
            }
            Type::Reference(mutable, t) => {
                let abi = ReferenceABI {
                    mutable,
                    ty: self.type_to_type_abi(*t, ty_args)?,
                };
                TypeABI::Reference(Box::new(abi))
            }
            _ => {
                bail!("cannot gen type abi for {:?}", ty0);
            }
        };
        Ok(abi)
    }
    fn compute_module_abi(&self, module_env: &ModuleEnv<'env>) -> anyhow::Result<ModuleABI> {
        let symbol_pool = module_env.symbol_pool();
        let mut struct_abis = vec![];
        for s in module_env.get_structs() {
            let ty_args: Vec<_> = s
                .get_named_type_parameters()
                .iter()
                .map(|ty_param| {
                    TypeArgumentABI::new(symbol_pool.string(ty_param.0).to_string().to_snake_case())
                })
                .collect();

            let mut fields = vec![];
            for f in s.get_fields() {
                let ft = f.get_type();
                let type_abi = self.type_to_type_abi(ft, &ty_args)?;
                fields.push(StructFieldABI {
                    name: symbol_pool.string(f.get_name()).to_string(),
                    ty: type_abi,
                })
            }

            let abi = StructDefinitionABI {
                name: symbol_pool.string(s.get_name()).to_string(),
                ty_args,
                doc: s.get_doc().to_string(),
                fields,
            };
            struct_abis.push(abi);
        }
        let mut func_abis = vec![];
        for func in module_env.get_functions() {
            // only output public functions.
            if !func.is_public() {
                continue;
            }

            let name = symbol_pool.string(func.get_name()).to_string();
            let doc = func.get_doc().to_string();
            let ty_args: Vec<_> = func
                .get_named_type_parameters()
                .iter()
                .map(|ty_param| {
                    TypeArgumentABI::new(symbol_pool.string(ty_param.0).to_string().to_snake_case())
                })
                .collect();

            let mut args = vec![];
            for p in func.get_parameters() {
                let name = symbol_pool.string(p.0).to_string();
                let ty = self.type_to_type_abi(p.1, &ty_args)?;
                args.push(FuncArgumentABI { name, ty });
            }
            let mut rets = vec![];
            for ret in func.get_return_types() {
                rets.push(self.type_to_type_abi(ret, &ty_args)?);
            }

            func_abis.push(FuncDefinitionABI {
                name,
                doc,
                ty_args,
                args,
                rets,
            });
        }

        let name = symbol_pool.string(module_env.get_name().name()).to_string();
        let address = *module_env.self_address();
        let doc = module_env.get_doc().to_string();
        Ok(ModuleABI {
            address,
            name,
            doc,
            structs: struct_abis,
            funcs: func_abis,
        })
    }

    fn load_compiled_bytes(&self, module_env: &ModuleEnv<'env>) -> anyhow::Result<Vec<u8>> {
        let mut path = PathBuf::from(&self.options.compiled_script_directory);
        path.push(
            PathBuf::from(module_env.get_source_path())
                .with_extension("mv")
                .file_name()
                .expect("file name"),
        );
        let mut f = match std::fs::File::open(path.clone()) {
            Ok(f) => f,
            Err(error) => bail!("Failed to open compiled file {:?}: {}", path, error),
        };
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    fn get_type_tag_skipping_references(&self, ty0: &ty::Type) -> anyhow::Result<Option<TypeTag>> {
        use ty::Type::*;
        let tag = match ty0 {
            Primitive(prim) => {
                use ty::PrimitiveType::*;
                match prim {
                    Bool => TypeTag::Bool,
                    U8 => TypeTag::U8,
                    U64 => TypeTag::U64,
                    U128 => TypeTag::U128,
                    Address => TypeTag::Address,
                    Signer => TypeTag::Signer,
                    Num | Range | TypeValue => bail!("Type {:?} is not allowed in scripts.", ty0),
                }
            }
            Reference(_, _) => {
                // Skip references (most likely a `&signer` type)
                return Ok(None);
            }
            Vector(ty) => {
                let tag = self.get_type_tag(ty)?;
                TypeTag::Vector(Box::new(tag))
            }
            Tuple(_)
            | Struct(_, _, _)
            | TypeParameter(_)
            | Fun(_, _)
            | TypeDomain(_)
            | TypeLocal(_)
            | Error
            | Var(_) => bail!("Type {:?} is not allowed in scripts.", ty0),
        };
        Ok(Some(tag))
    }

    fn get_type_tag(&self, ty: &ty::Type) -> anyhow::Result<TypeTag> {
        if let Some(tag) = self.get_type_tag_skipping_references(ty)? {
            return Ok(tag);
        }
        bail!(
            "References such as {:?} are only allowed in the list of parameters.",
            ty
        );
    }
}
