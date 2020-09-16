use libra_types::transaction::TypeArgumentABI;
use move_core_types::account_address::AccountAddress;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ModuleABI {
    pub address: AccountAddress,
    pub name: String,
    pub doc: String,
    pub structs: Vec<StructDefinitionABI>,
    pub funcs: Vec<FuncDefinitionABI>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructDefinitionABI {
    pub name: String,
    pub doc: String,
    pub ty_args: Vec<TypeArgumentABI>,
    pub fields: Vec<StructFieldABI>,
}
#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct FuncDefinitionABI {
    pub name: String,
    pub doc: String,
    pub ty_args: Vec<TypeArgumentABI>,
    pub args: Vec<FuncArgumentABI>,
    pub rets: Vec<TypeABI>,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct FuncArgumentABI {
    pub name: String,
    pub ty: TypeABI,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructFieldABI {
    pub name: String,
    pub ty: TypeABI,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum TypeABI {
    Bool,
    U8,
    U64,
    U128,
    Address,
    Signer,
    Vector(Box<TypeABI>),
    Struct(Box<StructABI>),
    Tuple(Vec<TypeABI>),
    TypeArgument(Box<TypeArgumentABI>),
    Reference(Box<ReferenceABI>),
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum PrimitiveABI {
    Bool,
    U8,
    U64,
    U128,
    Address,
    Signer,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ReferenceABI {
    pub mutable: bool,
    pub ty: TypeABI,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct StructABI {
    pub address: AccountAddress,
    pub name: String,
    pub module: String,
    pub ty_args: Vec<TypeABI>,
}
