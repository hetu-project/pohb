use std::collections::HashMap;

pub trait ClockClientContext {
    // clock value type, which usually consist a "causality part" for comparing and ordering and a
    // "proof part" for bring trust to both the order and a computation result
    type Clock; // whenever we say `Clock` we mean clock value
    type Output;

    // `Ok(())` when `clock` can ensure both the integrity of `output` i.e. it is the expected
    // result of the desired computation, and the integrity of the contained clock value itself,
    // i.e. the `clock` establishes a sound partial order to other `Clock` instances, while the
    // order is guaranteed to be aligned with causality
    // notice that neither the computation input nor (even) the computation itself is specified,
    // which has dual consequences
    // * the clock values can be verified with minimal context. the verifier doesn't need to learn
    //   any additional information in order to verify the clock values. this results in perfect
    //   transferrable verifiability as anyone who subscribe to the consensus infrastructure (a.k.a
    //   "the chain") can verify all clocks populated to the chain independently, without even know
    //   what computation tasks produce such clocks. this is especially desirable when the clock
    //   values potentially travels far and end up in foreign modulars carried by the attribution
    //   layer
    // * the fact that a clock value is verified may bring less/weaker trust than expected. for
    //   example it says nothing about *what* computation has been trustfully preformed, just *some*
    //   computation. the responsibility of ensuring the performed computation to be the expected
    //   one has been lifted to the point where the clock was being produced i.e.
    //   `ClockContext::prove` below. the `Clock`'s trust mechanism should ensure that the clock
    //   value is only permitted to be produced if the expected partial computation has been
    //   performed, instead of just "here is the performed computation i am recording and
    //   authenticating, take it if you like it (on your own risk)"
    // this two points originates from the inherent features of the inductive clock construction
    // procedure. they are not design choice but the musts to prevent evergrowing storage and
    // computation overheads related to clock values (well an exception could be made to the
    // computation description if we assume it to be static and finite, but i don't consider that
    // assumption to be desirable). the following one is indeed design choice though
    // * a verified clock value is not telling anything about who is producing the clock value
    // it is feasible to do the other way i.e. either one more peer id parameter or returning the
    // peer id, but it is not significant useful since it only asserts the peer who performed the
    // immediate preceding computation stage (and produced the clock value), and in the currently
    // imagined scenario we probably don't care who performed any stage including the last stage
    // at all
    fn verify(&self, clock: &Self::Clock, output: &Self::Output) -> anyhow::Result<()>;
}

pub trait ClockContext: ClockClientContext {
    type Input;

    // `Ok(clock)` only when both
    // * everything in `predecessors` is as expected. this probably means for all `(clock, input)`
    //   in `predecessors`, `self.verify(clock, input)` returned `Ok(())`
    // * `output` is the expected computation result of all inputs given in `predecessors`
    // the returned `clock` should be verifiable and happens after all clock values in
    // `predecessors`, i.e. `matches!(clock.partial_cmp(&other_clock), Some(Ordering::Greater))` for
    // all `other_clock` in `predecessors`
    // there may be more desired input for a clock context to produce a clock value e.g. peer's own
    // identity, the performed computation stage etc. those are considered as static data of a clock
    // context and should be passed in during initializing the context
    fn prove(
        &self,
        predecessors: &[(Self::Clock, Self::Input)],
        output: &Self::Output,
    ) -> anyhow::Result<Self::Clock>;
}

pub type NodeId = u32;

#[derive(Debug)]
pub struct OrdinaryClock(pub HashMap<NodeId, u32>);
