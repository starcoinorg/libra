address 0x1 {
module Token {
    /// Return Token's module address, module name, and type name of `E`.
    native public fun name_of<TokenType>(): (address, vector<u8>, vector<u8>);
}
}