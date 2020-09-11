address 0x2 {

module A {

    struct S {
        f1: u64,
    }

    public fun new(): 0x2::A::S { Self::S { f1: 20 } }

    public fun get_1(): u64 {
        let s = Self::new();
        s.f1
    }

    public fun get_2(): (u64, u64) {
        let s = Self::new();
        (s.f1, 1)
    }

    public fun get_s(): S {
        let s = Self::new();
        s
    }
}

}
