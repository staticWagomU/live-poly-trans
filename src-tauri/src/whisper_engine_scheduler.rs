#[derive(Debug, Clone)]
pub struct RollingTranscriptionScheduler {
    step_samples: usize,
    last_scheduled_total: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollingTranscriptionDecision {
    Wait,
    Transcribe,
    DropBacklog,
}

impl RollingTranscriptionScheduler {
    pub fn new(step_samples: usize) -> Self {
        Self {
            step_samples,
            last_scheduled_total: 0,
        }
    }

    pub fn observe_total_samples(&mut self, total_samples: usize) -> bool {
        self.observe_total_samples_with_backlog_limit(total_samples, usize::MAX)
            == RollingTranscriptionDecision::Transcribe
    }

    pub fn observe_total_samples_with_backlog_limit(
        &mut self,
        total_samples: usize,
        max_backlog_steps: usize,
    ) -> RollingTranscriptionDecision {
        if total_samples.saturating_sub(self.last_scheduled_total) < self.step_samples {
            return RollingTranscriptionDecision::Wait;
        }

        let backlog_steps =
            total_samples.saturating_sub(self.last_scheduled_total) / self.step_samples;
        self.last_scheduled_total = total_samples;
        if backlog_steps > max_backlog_steps {
            RollingTranscriptionDecision::DropBacklog
        } else {
            RollingTranscriptionDecision::Transcribe
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_after_each_step_of_new_samples() {
        let mut scheduler = RollingTranscriptionScheduler::new(4);

        assert!(!scheduler.observe_total_samples(3));
        assert!(scheduler.observe_total_samples(4));
        assert!(!scheduler.observe_total_samples(7));
        assert!(scheduler.observe_total_samples(8));
    }

    #[test]
    fn drops_rolling_work_when_backlog_exceeds_limit() {
        let mut scheduler = RollingTranscriptionScheduler::new(4);

        assert_eq!(
            scheduler.observe_total_samples_with_backlog_limit(12, 2),
            RollingTranscriptionDecision::DropBacklog
        );
        assert_eq!(
            scheduler.observe_total_samples_with_backlog_limit(15, 2),
            RollingTranscriptionDecision::Wait
        );
        assert_eq!(
            scheduler.observe_total_samples_with_backlog_limit(16, 2),
            RollingTranscriptionDecision::Transcribe
        );
    }
}
