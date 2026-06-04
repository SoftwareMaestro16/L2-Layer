import "@acton/emulation/network"
import "@acton/emulation/testing"
import "@acton/testing/expect"

import "@wrappers/Counter.gen"
import "@examples/l2-counter/counter"

get fun `test sample l2 counter deploy increment and getter`() {
    val user = testing.treasury("sample-l2-counter-user");
    val counter = Counter.fromStorage({ counter: 0 });

    val deploy = counter.deploy(user.address, { value: ton("0.1") });
    expect(deploy).toHaveSuccessfulDeploy({
        to: counter.address,
    });
    expect(counter.currentCounter()).toEqual(0);

    val increment = counter.sendCounterIncrement(user.address, 3, { value: ton("0.05") });
    expect(increment).toHaveSuccessfulTx<CounterIncrement>({
        from: user.address,
        to: counter.address,
    });
    expect(counter.currentCounter()).toEqual(3);
}

get fun `test sample l2 counter rejects zero increment`() {
    val user = testing.treasury("sample-l2-counter-zero-user");
    val counter = Counter.fromStorage({ counter: 1 });

    counter.deploy(user.address, { value: ton("0.1") });
    val rejected = counter.sendCounterIncrement(user.address, 0, { value: ton("0.05") });

    expect(rejected).toHaveFailedTx({
        from: user.address,
        to: counter.address,
        exitCode: 9001,
    });
    expect(counter.currentCounter()).toEqual(1);
}
