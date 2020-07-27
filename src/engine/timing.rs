use std::collections::HashMap;
use std::time::{Instant, Duration};

use mixlab_protocol::{ModuleId, PerformanceInfo, PerformanceAccount, PerformanceMetric, Microseconds};

use crate::engine;
use crate::util;

pub const TICK_BUDGET: Duration = Duration::from_micros(1_000_000 / engine::TICKS_PER_SECOND as u64);

pub struct EngineStat {
    is_realtime: bool,
    last_lagged: Option<Instant>,
    accounts: HashMap<PerformanceAccount, Stat>,
}

impl EngineStat {
    pub fn new() -> Self {
        EngineStat {
            is_realtime: false,
            last_lagged: None,
            accounts: HashMap::new(),
        }
    }

    pub fn record_tick<T>(&mut self, scheduled_tick_end: Instant, f: impl FnOnce(&mut TickStat) -> T) -> T {
        let start = Instant::now();
        let mut tick = TickStat::new(self);
        let retn = f(&mut tick);
        let end = Instant::now();

        tick.stat.is_realtime = end < scheduled_tick_end;

        let tick_time = end - start;

        if tick_time > TICK_BUDGET {
            tick.stat.last_lagged = Some(Instant::now());
            eprintln!("WARNING: tick ran over time! elapsed: {} us, budget: {} us", tick_time.as_micros(), TICK_BUDGET.as_micros());
        }

        tick.stat.add_sample(PerformanceAccount::Engine, tick_time - tick.modules_accounted_for);

        retn
    }

    pub fn report(&self) -> PerformanceInfo {
        let time_since_lag = self.last_lagged.map(|time| Instant::now() - time);

        PerformanceInfo {
            realtime: self.is_realtime,
            lag: util::temporal_warning(time_since_lag),
            tick_rate: engine::TICKS_PER_SECOND,
            tick_budget: Microseconds(TICK_BUDGET.as_micros() as u64),
            accounts: self.accounts.iter().map(|(account, stat)| {
                (*account, PerformanceMetric {
                    last: Microseconds(stat.last().as_micros() as u64),
                })
            }).collect()
        }
    }

    pub fn remove_module(&mut self, module_id: ModuleId) {
        self.accounts.remove(&PerformanceAccount::Module(module_id));
    }

    fn add_sample(&mut self, account: PerformanceAccount, sample: Duration) {
        self.accounts.entry(account)
            .and_modify(|stat| stat.add_sample(sample))
            .or_insert(Stat::with_initial_sample(sample));
    }
}

pub struct TickStat<'a> {
    stat: &'a mut EngineStat,
    modules_accounted_for: Duration,
}

impl<'a> TickStat<'a> {
    fn new(stat: &'a mut EngineStat) -> Self {
        TickStat {
            stat,
            modules_accounted_for: Duration::from_micros(0),
        }
    }

    pub fn record_module<T>(&mut self, module_id: ModuleId, f: impl FnOnce() -> T) -> T {
        let start = Instant::now();
        let retn = f();
        let end = Instant::now();
        let elapsed_time = end - start;
        self.modules_accounted_for += elapsed_time;
        self.stat.add_sample(PerformanceAccount::Module(module_id), elapsed_time);
        retn
    }
}

struct Stat {
    last: u128
}

impl Stat {
    pub fn with_initial_sample(sample: Duration) -> Self {
        Stat {
            last: sample.as_micros(),
        }
    }

    pub fn last(&self) -> Duration {
        Duration::from_micros(self.last as u64)
    }

    pub fn add_sample(&mut self, sample: Duration) {
        self.last = sample.as_micros();
    }
}
