/// # Safety
/// `HASHCU` computes the TVM representation hash of an immutable cell and has no side effects.
fun cellReprHash(data: cell): uint256
    asm "HASHCU"
