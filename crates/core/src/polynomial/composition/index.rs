// Copyright 2024 Ulvetanna Inc.

use crate::polynomial::{CompositionPoly, Error};
use binius_field::PackedField;
use binius_utils::bail;
use std::fmt::Debug;

/// An adapter which allows evaluating a composition over a larger query by indexing into it.
/// See [`index_composition`] for a factory method.
#[derive(Clone, Debug)]
pub struct IndexComposition<C, const N: usize> {
	/// Number of variables in a larger query
	n_vars: usize,
	/// Mapping from the inner composition query variables to outer query variables
	indices: [usize; N],
	/// Inner composition
	composition: C,
}

impl<P: PackedField, C: CompositionPoly<P>, const N: usize> CompositionPoly<P>
	for IndexComposition<C, N>
{
	fn n_vars(&self) -> usize {
		self.n_vars
	}

	fn degree(&self) -> usize {
		self.composition.degree()
	}

	fn evaluate(&self, query: &[P]) -> Result<P, Error> {
		if query.len() != self.n_vars {
			bail!(Error::IncorrectQuerySize {
				expected: self.n_vars,
			});
		}

		let subquery = self.indices.map(|index| query[index]);
		self.composition.evaluate(&subquery)
	}

	fn binary_tower_level(&self) -> usize {
		self.composition.binary_tower_level()
	}
}

/// A factory helper method to create an [`IndexComposition`] by looking at
///  * `superset` - a set of identifiers of a greater (outer) query
///  * `subset` - a set of identifiers of a smaller query, the one which corresponds to the inner composition directly
///
/// Identifiers may be anything `Eq` - `OracleId`, `MultilinearPolyOracle<F>`, etc.
pub fn index_composition<E, C, const N: usize>(
	superset: &[E],
	subset: [E; N],
	composition: C,
) -> Result<IndexComposition<C, N>, Error>
where
	E: PartialEq,
{
	let n_vars = superset.len();

	// array_try_map is unstable as of 03/24, check the condition beforehand
	let proper_subset = subset.iter().all(|subset_item| {
		superset
			.iter()
			.any(|superset_item| superset_item == subset_item)
	});

	if !proper_subset {
		bail!(Error::MixedMultilinearNotFound);
	}

	let indices = subset.map(|subset_item| {
		superset
			.iter()
			.position(|superset_item| superset_item == &subset_item)
			.expect("Superset condition checked above.")
	});

	Ok(IndexComposition {
		n_vars,
		indices,
		composition,
	})
}
