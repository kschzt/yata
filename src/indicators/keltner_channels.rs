#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::core::{IndicatorConfig, IndicatorInitializer, IndicatorInstance, IndicatorResult};
use crate::core::{Method, PeriodType, Source, ValueType, OHLC};
use crate::helpers::{method, RegularMethod, RegularMethods};
use crate::methods::{CrossAbove, CrossUnder, SMA};

// https://en.wikipedia.org/wiki/Keltner_channel
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct KeltnerChannels {
	pub period: PeriodType,
	pub method: RegularMethods,
	pub sigma: ValueType,
	pub source: Source,
}

impl IndicatorConfig for KeltnerChannels {
	fn validate(&self) -> bool {
		self.period > 1 && self.sigma > 1e-4
	}

	fn set(&mut self, name: &str, value: String) {
		match name {
			"period" => self.period = value.parse().unwrap(),
			"method" => self.method = value.parse().unwrap(),
			"sigma" => self.sigma = value.parse().unwrap(),
			"source" => self.source = value.parse().unwrap(),

			_ => {
				dbg!(format!(
					"Unknown attribute `{:}` with value `{:}` for `{:}`",
					name,
					value,
					std::any::type_name::<Self>(),
				));
			}
		};
	}

	fn size(&self) -> (u8, u8) {
		(3, 1)
	}
}

impl<T: OHLC> IndicatorInitializer<T> for KeltnerChannels {
	type Instance = KeltnerChannelsInstance<T>;

	fn init(self, candle: T) -> Self::Instance
	where
		Self: Sized,
	{
		let cfg = self;
		let src = candle.source(cfg.source);
		Self::Instance {
			prev_candle: candle,
			ma: method(cfg.method, cfg.period, src),
			sma: SMA::new(cfg.period, candle.high() - candle.low()),
			cross_above: CrossAbove::default(),
			cross_under: CrossUnder::default(),
			cfg,
		}
	}
}

impl Default for KeltnerChannels {
	fn default() -> Self {
		Self {
			period: 20,
			sigma: 1.0,
			source: Source::Close,
			method: RegularMethods::EMA,
		}
	}
}

#[derive(Debug)]
pub struct KeltnerChannelsInstance<T: OHLC> {
	cfg: KeltnerChannels,

	prev_candle: T,
	ma: RegularMethod,
	sma: SMA,
	cross_above: CrossAbove,
	cross_under: CrossUnder,
}

impl<T: OHLC> IndicatorInstance<T> for KeltnerChannelsInstance<T> {
	type Config = KeltnerChannels;

	fn name(&self) -> &str {
		"KeltnerChannels"
	}

	#[inline]
	fn config(&self) -> &Self::Config {
		&self.cfg
	}

	fn next(&mut self, candle: T) -> IndicatorResult {
		let source = candle.source(self.cfg.source);
		let tr = candle.tr(&self.prev_candle);
		let ma: ValueType = self.ma.next(source);
		let atr = self.sma.next(tr);

		let upper = ma + atr * self.cfg.sigma;
		let lower = ma - atr * self.cfg.sigma;

		let signal =
			self.cross_under.next((source, lower)) - self.cross_above.next((source, upper));

		IndicatorResult::new(&[source, upper, lower], &[signal])
	}
}