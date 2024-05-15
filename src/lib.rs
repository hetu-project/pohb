use std::{cmp::Ordering, collections::HashMap, marker::PhantomData};

use derive_more::{Deref, DerefMut};
use derive_where::derive_where;
use serde::{Deserialize, Serialize};

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
        predecessors: &[(&Self::Clock, &Self::Input)],
        output: &Self::Output,
    ) -> anyhow::Result<Self::Clock>;
    // TODO make this into an asynchronous interface, as the clock proving may not be instant
    // current stabilized async trait method is crappy, i would prefer to add a closure parameter
    // and pass a oneshot sender with it
}

// id of the computation nodes
// in this prototype clients and the centralized "communication hub" have no id
pub type NodeId = u32;

pub type TaskId = u32;

// the untrusted reference clock that lacks the "proof part"
// not suitable for directly used, but can be composed as the "causality part"
// i.e. the be delegated for implementing `PartialOrd`
#[derive(Debug, Clone, Default, Deref, DerefMut, Serialize, Deserialize)]
pub struct OrdinaryClock(pub HashMap<NodeId, u32>);

impl OrdinaryClock {
    pub fn new_genesis() -> Self {
        Self::default()
    }

    pub fn new<'a>(deps: impl Iterator<Item = &'a Self>, id: NodeId) -> Self {
        let mut value = HashMap::new();
        for dep in deps {
            for (other_id, seq) in &**dep {
                let the_seq = value.entry(*other_id).or_default();
                *the_seq = u32::max(*the_seq, *seq) + (*other_id == id) as u32
            }
        }
        Self(value)
    }

    pub fn is_genesis(&self) -> bool {
        self.values().all(|seq| *seq == 0)
    }
}

impl PartialOrd for OrdinaryClock {
    fn ge(&self, other: &Self) -> bool {
        other.iter().all(|(other_id, other_seq)| {
            self.get(other_id).copied().unwrap_or_default() >= *other_seq
        })
    }

    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self.ge(other), other.ge(self)) {
            (true, true) => Some(Ordering::Equal),
            (true, false) => Some(Ordering::Greater),
            (false, true) => Some(Ordering::Less),
            (false, false) => None,
        }
    }
}

impl PartialEq for OrdinaryClock {
    fn eq(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Equal))
    }
}

#[derive(Debug)]
#[derive_where(Default)]
pub struct OrdinaryClientContext<O>(PhantomData<O>);

impl<O> OrdinaryClientContext<O> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<O> ClockClientContext for OrdinaryClientContext<O> {
    type Clock = OrdinaryClock;
    type Output = O;

    fn verify(&self, _: &Self::Clock, &_: &Self::Output) -> anyhow::Result<()> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct OrdinaryContext<I, O>(NodeId, PhantomData<(I, O)>);

impl<I, O> OrdinaryContext<I, O> {
    pub fn new(id: NodeId) -> Self {
        Self(id, Default::default())
    }
}

impl<I, O> ClockClientContext for OrdinaryContext<I, O> {
    type Clock = OrdinaryClock;
    type Output = O;

    fn verify(&self, _: &Self::Clock, &_: &Self::Output) -> anyhow::Result<()> {
        Ok(())
    }
}

impl<I, O> ClockContext for OrdinaryContext<I, O> {
    type Input = I;

    fn prove(
        &self,
        predecessors: &[(&Self::Clock, &Self::Input)],
        _: &Self::Output,
    ) -> anyhow::Result<Self::Clock> {
        Ok(OrdinaryClock::new(
            predecessors.iter().map(|(clock, _)| *clock),
            self.0,
        ))
    }
}

// TODO extend into a DAG (or even general graph) representation
#[derive(Debug, Deserialize)]
pub struct Workflow {
    pub stages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StageSource {
    Start,
    Name(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskStage<C, I> {
    pub id: TaskId,
    pub source: StageSource,
    pub input: I,
    pub clocks: HashMap<String, C>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult<C, O> {
    pub id: TaskId,
    pub output: O,
    pub clocks: HashMap<String, C>,
}

fn verify<C: PartialOrd, O>(
    clocks: &HashMap<String, C>,
    output_stage: &str,
    output: &O,
    task: &Workflow,
    context: &impl ClockClientContext<Clock = C, Output = O>,
) -> anyhow::Result<()> {
    for window in task.stages.windows(2) {
        let [stage, next_stage] = window else {
            unreachable!()
        };
        let clock = clocks
            .get(stage)
            .ok_or(anyhow::format_err!("missing clock value of stage {stage}"))?;
        let next_clock = clocks.get(next_stage).ok_or(anyhow::format_err!(
            "missing clock value of stage {next_stage}"
        ))?;
        anyhow::ensure!(matches!(
            next_clock.partial_cmp(clock),
            Some(Ordering::Greater)
        ));
        // we only need to verify the last clock value, and we also can only verify the last clock
        // value: we don't have the necessary immediate results to verify the other clocks
        // just verify the last clock value is enough to ensure correct `task_result.output`, as
        // already discussed in comments of `ClockClientContext`
        // notice that although we have checked whether the other clocks happen before the clocks of
        // successive stages, this is not enough for asserting those are the clocks that eventually
        // lead to the last clock value i.e. producing the last clock value has made use of all/any
        // of them (really? cannot say for sure), because we don't even know whether those clocks
        // are verifiable or not. so including those clock are kind of pointless under current setup
        if next_stage == output_stage {
            context.verify(next_clock, output)?
        }
    }
    Ok(())
}

impl<C: PartialOrd, I> TaskStage<C, I> {
    pub fn verify(
        &self,
        task: &Workflow,
        context: &impl ClockClientContext<Clock = C, Output = I>,
    ) -> anyhow::Result<()> {
        match &self.source {
            StageSource::Start => Ok(()),
            StageSource::Name(last_stage) => {
                verify(&self.clocks, last_stage, &self.input, task, context)
            }
        }
    }
}

impl<C: PartialOrd, O> TaskResult<C, O> {
    pub fn verify(
        &self,
        task: &Workflow,
        context: &impl ClockClientContext<Clock = C, Output = O>,
    ) -> anyhow::Result<()> {
        match task.stages.last() {
            None => Ok(()),
            Some(last_stage) => verify(&self.clocks, last_stage, &self.output, task, context),
        }
    }
}
