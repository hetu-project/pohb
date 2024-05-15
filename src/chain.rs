use std::cmp::Ordering::Greater;

use crate::{ClockClientContext, TaskResult, Workflow};

pub fn verify<C: PartialOrd, O>(
    task_result: &TaskResult<C, O>,
    task: &Workflow,
    context: &impl ClockClientContext<Clock = C, Output = O>,
) -> anyhow::Result<()> {
    for window in task.stages.windows(2) {
        let [stage, next_stage] = window else {
            unreachable!()
        };
        let clock = task_result
            .clocks
            .get(stage)
            .ok_or(anyhow::format_err!("missing clock value of stage {stage}"))?;
        let next_clock = task_result
            .clocks
            .get(next_stage)
            .ok_or(anyhow::format_err!(
                "missing clock value of stage {next_stage}"
            ))?;
        anyhow::ensure!(matches!(next_clock.partial_cmp(clock), Some(Greater)));
        // we only need to verify the last clock value, and we also can only verify the last clock
        // value: we don't have the necessary immediate results to verify the other clocks
        // just verify the last clock value is enough to ensure correct `task_result.output`, as
        // already discussed in comments of `ClockClientContext`
        // notice that although we have checked whether the other clocks happen before the clocks of
        // successive stages, this is not enough for asserting those are the clocks that eventually
        // lead to the last clock value i.e. producing the last clock value has made use of all/any
        // of them (really? cannot say for sure), because we don't even know whether those clocks
        // are verifiable or not. so including those clock are kind of pointless under current setup
        if Some(next_stage) == task.stages.last() {
            context.verify(next_clock, &task_result.output)?
        }
    }
    Ok(())
}
