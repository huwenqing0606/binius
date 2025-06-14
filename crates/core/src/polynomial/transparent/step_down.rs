// Copyright 2024 Ulvetanna Inc.

use crate::polynomial::{Error, MultilinearExtension, MultivariatePoly};
use binius_field::{BinaryField1b, Field, PackedField};
use binius_utils::bail;

/// Represents a multilinear F2-polynomial whose evaluations over the hypercube are 1 until a
/// specified index where they change to 0.
///
/// ```txt
///     (1 << n_vars)
/// <-------------------->
/// 1,1 .. 1,1,0,0, .. 0,0
///            ^
///            index of first 0
/// ```
///
/// This is useful for making constraints that are not enforced at the last rows of the trace
#[derive(Debug, Clone)]
pub struct StepDown {
	n_vars: usize,
	index: usize,
}

impl StepDown {
	pub fn new(n_vars: usize, index: usize) -> Result<Self, Error> {
		if index < 1 || index >= (1 << n_vars) {
			bail!(Error::ArgumentRangeError {
				arg: "index".into(),
				range: 1..(1 << n_vars),
			})
		} else {
			Ok(Self { n_vars, index })
		}
	}

	pub fn multilinear_extension<P: PackedField<Scalar = BinaryField1b>>(
		&self,
	) -> Result<MultilinearExtension<P>, Error> {
		if self.n_vars < P::LOG_WIDTH {
			bail!(Error::PackedFieldNotFilled {
				length: 1 << self.n_vars,
				packed_width: 1 << P::LOG_WIDTH,
			});
		}
		let log_packed_length = self.n_vars - P::LOG_WIDTH;
		let packed_index = self.index / P::WIDTH;
		let mut result = vec![P::zero(); 1 << log_packed_length];
		result[..packed_index].fill(P::one());
		for i in 0..self.index % P::WIDTH {
			result[packed_index].set(i, P::Scalar::ONE);
		}
		MultilinearExtension::from_values(result)
	}
}

impl<F: Field> MultivariatePoly<F> for StepDown {
	fn degree(&self) -> usize {
		self.n_vars
	}

	fn n_vars(&self) -> usize {
		self.n_vars
	}

	fn evaluate(&self, query: &[F]) -> Result<F, Error> {
		let n_vars = MultivariatePoly::<F>::n_vars(self);
		if query.len() != n_vars {
			bail!(Error::IncorrectQuerySize { expected: n_vars });
		}
		let mut k = self.index;

		// `result` is the evaluation of the complimentary "step-up" function that is 0 at indices 0..self.index and 1
		// at indices self.index..2^n. The "step-down" evaluation is then 1 - `result`.
		let mut result = F::ONE;
		for q in query {
			if k & 1 == 1 {
				// interpolate a line that is 0 at 0 and `result` at 1, at the point q
				result *= q;
			} else {
				// interpolate a line that is `result` at 0 and 1 at 1, and evaluate at q
				result = result * (F::ONE - q) + q;
			}
			k >>= 1;
		}

		Ok(F::ONE - result)
	}

	fn binary_tower_level(&self) -> usize {
		0
	}
}

#[cfg(test)]
mod tests {
	use super::StepDown;
	use crate::protocols::test_utils::{hypercube_evals_from_oracle, macros::felts, packed_slice};
	use binius_field::{
		BinaryField1b, PackedBinaryField128x1b, PackedBinaryField256x1b, PackedField,
	};

	#[test]
	fn test_step_down_trace_without_packing_simple_cases() {
		assert_eq!(stepdown_evals::<BinaryField1b>(2, 1), felts!(BinaryField1b[1, 0, 0, 0]));
		assert_eq!(stepdown_evals::<BinaryField1b>(2, 2), felts!(BinaryField1b[1, 1, 0, 0]));
		assert_eq!(stepdown_evals::<BinaryField1b>(2, 3), felts!(BinaryField1b[1, 1, 1, 0]));
		assert_eq!(
			stepdown_evals::<BinaryField1b>(3, 1),
			felts!(BinaryField1b[1, 0, 0, 0, 0, 0, 0, 0])
		);
		assert_eq!(
			stepdown_evals::<BinaryField1b>(3, 2),
			felts!(BinaryField1b[1, 1, 0, 0, 0, 0, 0, 0])
		);
		assert_eq!(
			stepdown_evals::<BinaryField1b>(3, 3),
			felts!(BinaryField1b[1, 1, 1, 0, 0, 0, 0, 0])
		);
		assert_eq!(
			stepdown_evals::<BinaryField1b>(3, 4),
			felts!(BinaryField1b[1, 1, 1, 1, 0, 0, 0, 0])
		);
		assert_eq!(
			stepdown_evals::<BinaryField1b>(3, 5),
			felts!(BinaryField1b[1, 1, 1, 1, 1, 0, 0, 0])
		);
		assert_eq!(
			stepdown_evals::<BinaryField1b>(3, 6),
			felts!(BinaryField1b[1, 1, 1, 1, 1, 1, 0, 0])
		);
		assert_eq!(
			stepdown_evals::<BinaryField1b>(3, 7),
			felts!(BinaryField1b[1, 1, 1, 1, 1, 1, 1, 0])
		);
	}

	#[test]
	fn test_step_down_trace_without_packing() {
		assert_eq!(
			stepdown_evals::<BinaryField1b>(9, 314),
			packed_slice::<BinaryField1b>(&[(0..314, 1), (314..512, 0)])
		);
		assert_eq!(
			stepdown_evals::<BinaryField1b>(10, 555),
			packed_slice::<BinaryField1b>(&[(0..555, 1), (555..1024, 0)])
		);
		assert_eq!(
			stepdown_evals::<BinaryField1b>(11, 1),
			packed_slice::<BinaryField1b>(&[(0..1, 1), (1..2048, 0)])
		);
	}

	#[test]
	fn test_step_down_trace_with_packing_128() {
		assert_eq!(
			stepdown_evals::<PackedBinaryField128x1b>(9, 314),
			packed_slice::<PackedBinaryField128x1b>(&[(0..314, 1), (314..512, 0)])
		);
		assert_eq!(
			stepdown_evals::<PackedBinaryField128x1b>(10, 555),
			packed_slice::<PackedBinaryField128x1b>(&[(0..555, 1), (555..1024, 0)])
		);
		assert_eq!(
			stepdown_evals::<PackedBinaryField128x1b>(11, 1),
			packed_slice::<PackedBinaryField128x1b>(&[(0..1, 1), (1..2048, 0)])
		);
	}

	#[test]
	fn test_step_down_trace_with_packing_256() {
		assert_eq!(
			stepdown_evals::<PackedBinaryField256x1b>(9, 314),
			packed_slice::<PackedBinaryField256x1b>(&[(0..314, 1), (314..512, 0)])
		);
		assert_eq!(
			stepdown_evals::<PackedBinaryField256x1b>(10, 555),
			packed_slice::<PackedBinaryField256x1b>(&[(0..555, 1), (555..1024, 0)])
		);
		assert_eq!(
			stepdown_evals::<PackedBinaryField256x1b>(11, 1),
			packed_slice::<PackedBinaryField256x1b>(&[(0..1, 1), (1..2048, 0)])
		);
	}

	#[test]
	fn test_consistency_between_multilinear_extension_and_multilinear_poly_oracle() {
		for n_vars in 1..5 {
			for index in 1..(1 << n_vars) {
				let step_down = StepDown::new(n_vars, index).unwrap();
				assert_eq!(
					hypercube_evals_from_oracle::<BinaryField1b>(&step_down),
					step_down
						.multilinear_extension::<BinaryField1b>()
						.unwrap()
						.evals()
				);
			}
		}
	}

	fn stepdown_evals<P>(n_vars: usize, index: usize) -> Vec<P>
	where
		P: PackedField<Scalar = BinaryField1b>,
	{
		StepDown::new(n_vars, index)
			.unwrap()
			.multilinear_extension::<P>()
			.unwrap()
			.evals()
			.to_vec()
	}
}
