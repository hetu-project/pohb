# Proof of Happen Before

Start the "simulated" network (including both gossip network and consensus infrastructure)

```
$ cargo run --bin network -- task.json
```

The `task.json` is used by the consensus policy e.g. smart contract to verify task results before working on it for consensus.

Open three more shells, run a computation node for each stage of the computation (specified in `task.json`) in each of them

```
$ cargo run --bin compute -- rand
$ cargo run --bin compute -- prod
$ cargo run --bin compute -- hash
```

Open one last shell and submit a computation task

```
$ cargo run --bin client
```

The result can be cross checked by pipelining the computation stages directly

```
$ echo -n hello | ./scripts/rand | ./scripts/prod | ./scripts/hash | hexdump -C
```

**Roadmap.**

* Fill the stage scripts with some useful serious machine learning.
* Secure the logical clock with Nitro Enclaves.