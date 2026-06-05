import "errors"
import "messages"

/// Put a raw OutActionList into TVM c5 after validation.
// acton-disable-next-line asm-function-missing-safety-comment
fun setTvmRegisterC5(actionList: cell): void
    asm "c5 POP"

/// Transform an ordinary or exotic cell into a slice as if it were ordinary.
// acton-disable-next-line asm-function-missing-safety-comment
fun OutActionsCell.beginParseAllowExotic(self): slice
    asm "XCTOS" "DROP"

fun OutActionsCell.verifyC5Actions(self, isExternal: bool): self {
    var count = 0;
    var cs = self.beginParseAllowExotic();

    do {
        val (nBits, nRefs) = cs.remainingBitsAndRefsCount();
        assert (nRefs == 2) throw Errors.InvalidC5;
        assert (nBits == 32 + 8) throw Errors.InvalidC5;

        val outAction = lazy OutActionWithSendMessageOnly.fromSlice(
            cs,
            { throwIfOpcodeDoesNotMatch: Errors.InvalidC5 },
        );

        if (isExternal) {
            assert (
                (outAction.sendMode & SEND_MODE_IGNORE_ERRORS) != 0
            ) throw Errors.ExternalSendMessageMustHaveIgnoreErrorsSendMode;
        }

        cs = outAction.prev.beginParseAllowExotic();
        count += 1;
    } while (!cs.isEmpty());

    assert (count <= 255) throw Errors.InvalidC5;
    return self;
}
