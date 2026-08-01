#[derive(Debug, Clone)]
pub struct RollingTranscriptionScheduler {
    step_samples: usize,
    last_scheduled_total: usize,
}

impl RollingTranscriptionScheduler {
    pub fn new(step_samples: usize) -> Self {
        Self {
            step_samples,
            last_scheduled_total: 0,
        }
    }

    pub fn observe_total_samples(&mut self, total_samples: usize) -> bool {
        if total_samples.saturating_sub(self.last_scheduled_total) < self.step_samples {
            return false;
        }

        self.last_scheduled_total = total_samples;
        true
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
}
