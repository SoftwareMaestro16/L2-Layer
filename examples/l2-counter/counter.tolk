struct (0x534c3201) CounterIncrement {
    by: uint32
}

type CounterMessage = CounterIncrement

struct CounterStorage {
    counter: uint64
}

fun CounterStorage.load(): CounterStorage {
    return CounterStorage.fromCell(contract.getData());
}

fun CounterStorage.save(self) {
    contract.setData(self.toCell());
}

contract SampleL2Counter {
    author: "Fabolous @SoftwareMaestro"
    version: "0.0.1"
    description: "Bounded sample L2 counter contract for Entropis TVM prototype demos."
    storage: CounterStorage
    incomingMessages: CounterMessage
}

fun onInternalMessage(in: InMessage) {
    val msg = lazy CounterMessage.fromSlice(in.body);
    match (msg) {
        CounterIncrement => {
            assert (msg.by > 0) throw 9001;
            var storage = lazy CounterStorage.load();
            storage.counter += msg.by;
            storage.save();
        }
        else => {
            assert (in.body.isEmpty()) throw 0xffff;
        }
    }
}

/// Reads the current sample counter value.
get fun currentCounter(): uint64 {
    val storage = lazy CounterStorage.load();
    return storage.counter;
}
