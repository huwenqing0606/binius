// Copyright 2023 Ulvetanna Inc.

use super::util::tensor_prod_eq_ind;
use crate::polynomial::Error as PolynomialError;
use binius_field::{Field, PackedField};
use binius_utils::bail;
use bytemuck::zeroed_vec;
use std::cmp::max;

/// Tensor product expansion of sumcheck round challenges.
///
/// Stores the tensor product expansion $\bigotimes_{i = 0}^{n - 1} (1 - r_i, r_i)$
/// when `round()` is `n` for the sequence of sumcheck challenges $(r_0, ..., r_{n-1})$.
/// The tensor product can be updated with a new round challenge in linear time.
/// This is used in the first several rounds of the sumcheck prover for small-field polynomials,
/// before it becomes more efficient to switch over to the method that store folded multilinears.
#[derive(Debug, Clone)]
pub struct MultilinearQuery<P: PackedField> {
	expanded_query: Vec<P>,
	// We want to avoid initializing data at the moment when vector is growing,
	// So we allocate zeroed vector and keep track of the length of the initialized part.
	expanded_query_len: usize,
	n_vars: usize,
}

impl<P: PackedField> MultilinearQuery<P> {
	pub fn new(max_query_vars: usize) -> Result<Self, PolynomialError> {
		if max_query_vars > 31 {
			bail!(PolynomialError::TooManyVariables)
		} else {
			let len = max((1 << max_query_vars) / P::WIDTH, 1);
			let mut expanded_query = zeroed_vec(len);
			expanded_query[0] = P::set_single(P::Scalar::ONE);
			Ok(Self {
				expanded_query,
				expanded_query_len: 1,
				n_vars: 0,
			})
		}
	}

	pub fn with_full_query(query: &[P::Scalar]) -> Result<Self, PolynomialError> {
		Self::new(query.len())?.update(query)
	}

	pub fn n_vars(&self) -> usize {
		self.n_vars
	}

	/// Returns the tensor product expansion of the query
	///
	/// If the number of query variables is less than the packing width, return a single packed element.
	pub fn expansion(&self) -> &[P] {
		&self.expanded_query[0..self.expanded_query_len]
	}

	pub fn into_expansion(mut self) -> Vec<P> {
		// Trim query vector to the actual size
		self.expanded_query
			.resize(self.expanded_query_len, P::zero());

		self.expanded_query
	}

	pub fn update(
		mut self,
		extra_query_coordinates: &[P::Scalar],
	) -> Result<Self, PolynomialError> {
		let old_n_vars = self.n_vars;
		let new_n_vars = old_n_vars + extra_query_coordinates.len();
		let new_length = max((1 << new_n_vars) / P::WIDTH, 1);
		if new_length > self.expanded_query.len() {
			bail!(PolynomialError::MultilinearQueryFull {
				max_query_vars: old_n_vars,
			});
		}
		tensor_prod_eq_ind(
			old_n_vars,
			&mut self.expanded_query[..new_length],
			extra_query_coordinates,
		)?;

		Ok(Self {
			expanded_query: self.expanded_query,
			expanded_query_len: new_length,
			n_vars: new_n_vars,
		})
	}
}

#[cfg(test)]
mod tests {
	use super::MultilinearQuery;
	use crate::protocols::test_utils::macros::felts;

	macro_rules! expand_query {
		($f:ident[$($elem:expr),* $(,)?], Packing=$p:ident) => {
			binius_field::packed::iter_packed_slice(MultilinearQuery::<$p>::with_full_query(&[$($f::new($elem)),*]).unwrap().expansion()).collect::<Vec<_>>()
		};
	}

	#[test]
	fn test_query_no_packing_32b() {
		use binius_field::BinaryField32b;
		assert_eq!(
			expand_query!(BinaryField32b[], Packing = BinaryField32b),
			felts!(BinaryField32b[1])
		);
		assert_eq!(
			expand_query!(BinaryField32b[2], Packing = BinaryField32b),
			felts!(BinaryField32b[3, 2])
		);
		assert_eq!(
			expand_query!(BinaryField32b[2, 2], Packing = BinaryField32b),
			felts!(BinaryField32b[2, 1, 1, 3])
		);
		assert_eq!(
			expand_query!(BinaryField32b[2, 2, 2], Packing = BinaryField32b),
			felts!(BinaryField32b[1, 3, 3, 2, 3, 2, 2, 1])
		);
		assert_eq!(
			expand_query!(BinaryField32b[2, 2, 2, 2], Packing = BinaryField32b),
			felts!(BinaryField32b[3, 2, 2, 1, 2, 1, 1, 3, 2, 1, 1, 3, 1, 3, 3, 2])
		);
	}

	#[test]
	fn test_query_packing_4x32b() {
		use binius_field::{BinaryField32b, PackedBinaryField4x32b};
		assert_eq!(
			expand_query!(BinaryField32b[], Packing = PackedBinaryField4x32b),
			felts!(BinaryField32b[1, 0, 0, 0])
		);
		assert_eq!(
			expand_query!(BinaryField32b[2], Packing = PackedBinaryField4x32b),
			felts!(BinaryField32b[3, 2, 0, 0])
		);
		assert_eq!(
			expand_query!(BinaryField32b[2, 2], Packing = PackedBinaryField4x32b),
			felts!(BinaryField32b[2, 1, 1, 3])
		);
		assert_eq!(
			expand_query!(BinaryField32b[2, 2, 2], Packing = PackedBinaryField4x32b),
			felts!(BinaryField32b[1, 3, 3, 2, 3, 2, 2, 1])
		);
		assert_eq!(
			expand_query!(BinaryField32b[2, 2, 2, 2], Packing = PackedBinaryField4x32b),
			felts!(BinaryField32b[3, 2, 2, 1, 2, 1, 1, 3, 2, 1, 1, 3, 1, 3, 3, 2])
		);
	}

	#[test]
	fn test_query_packing_8x16b() {
		use binius_field::{BinaryField16b, PackedBinaryField8x16b};
		assert_eq!(
			expand_query!(BinaryField16b[], Packing = PackedBinaryField8x16b),
			felts!(BinaryField16b[1, 0, 0, 0, 0, 0, 0, 0])
		);
		assert_eq!(
			expand_query!(BinaryField16b[2], Packing = PackedBinaryField8x16b),
			felts!(BinaryField16b[3, 2, 0, 0, 0, 0, 0, 0])
		);
		assert_eq!(
			expand_query!(BinaryField16b[2, 2], Packing = PackedBinaryField8x16b),
			felts!(BinaryField16b[2, 1, 1, 3, 0, 0, 0, 0])
		);
		assert_eq!(
			expand_query!(BinaryField16b[2, 2, 2], Packing = PackedBinaryField8x16b),
			felts!(BinaryField16b[1, 3, 3, 2, 3, 2, 2, 1])
		);
		assert_eq!(
			expand_query!(BinaryField16b[2, 2, 2, 2], Packing = PackedBinaryField8x16b),
			felts!(BinaryField16b[3, 2, 2, 1, 2, 1, 1, 3, 2, 1, 1, 3, 1, 3, 3, 2])
		);
	}
}
