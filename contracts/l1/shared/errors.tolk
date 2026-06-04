enum Errors {
    UnknownOpcode = 0xffff
    Unauthorized = 0x1001
    Paused = 0x1002
    BadBatchNo = 0x1003
    BadPrevRoot = 0x1004
    BatchNotFound = 0x1005
    ChallengeWindow = 0x1006
    AlreadyFinalized = 0x1007
    NotFinalized = 0x1008
    AlreadyClaimed = 0x1009
    BadWithdrawalProof = 0x100a
    BadDepositValue = 0x100b
    UnsupportedJettonDeposit = 0x100c
}
