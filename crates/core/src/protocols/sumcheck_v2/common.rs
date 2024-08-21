// Copyright 2024 Ulvetanna Inc.

use super::error::Error;
use crate::polynomial::CompositionPoly;
use binius_field::Field;
use binius_utils::bail;
use getset::{CopyGetters, Getters};
use std::ops::{Add, AddAssign, Mul, MulAssign};

/// A claim about the sum of the values of a multilinear composite polynomial over the boolean
/// hypercube.
///
/// This struct contains a composition polynomial and a claimed sum and implicitly refers to a
/// sequence of multilinears that are composed. This is typically embedded within a
/// [`SumcheckClaim`], which contains more metadata about the multilinears (eg. the number of
/// variables they are defined over).
#[derive(Debug, Clone, Getters, CopyGetters)]
pub struct CompositeSumClaim<F: Field, Composition> {
	pub composition: Composition,
	pub sum: F,
}

/// A group of claims about the sum of the values of multilinear composite polynomials over the
/// boolean hypercube.
///
/// All polynomials in the group of claims are compositions of the same sequence of multilinear
/// polynomials. By defining [`SumcheckClaim`] in this way, the sumcheck protocol can implement
/// efficient batch proving and verification and reduce to a set of multilinear evaluations of the
/// same polynomials. In other words, this grouping deduplicates prover work and proof data that
/// would be redundant in a more naive implementation.
#[derive(Debug, CopyGetters)]
pub struct SumcheckClaim<F: Field, C> {
	#[getset(get_copy = "pub")]
	n_vars: usize,
	#[getset(get_copy = "pub")]
	n_multilinears: usize,
	composite_sums: Vec<CompositeSumClaim<F, C>>,
}

impl<F: Field, Composition> SumcheckClaim<F, Composition>
where
	Composition: CompositionPoly<F>,
{
	/// Constructs a new sumcheck claim.
	///
	/// ## Throws
	///
	/// * [`Error::InvalidComposition`] if any of the composition polynomials in the composite
	///   claims vector do not have their number of variables equal to `n_multilinears`
	pub fn new(
		n_vars: usize,
		n_multilinears: usize,
		composite_sums: Vec<CompositeSumClaim<F, Composition>>,
	) -> Result<Self, Error> {
		for CompositeSumClaim {
			ref composition, ..
		} in composite_sums.iter()
		{
			if composition.n_vars() != n_multilinears {
				bail!(Error::InvalidComposition {
					expected_n_vars: n_multilinears,
				});
			}
		}
		Ok(Self {
			n_vars,
			n_multilinears,
			composite_sums,
		})
	}

	/// Returns the maximum individual degree of all composite polynomials.
	pub fn max_individual_degree(&self) -> usize {
		self.composite_sums
			.iter()
			.map(|composite_sum| composite_sum.composition.n_vars())
			.max()
			.unwrap_or(0)
	}

	pub fn composite_sums(&self) -> &[CompositeSumClaim<F, Composition>] {
		&self.composite_sums
	}
}

/// A univariate polynomial in monomial basis.
///
/// The coefficient at position `i` in the inner vector corresponds to the term $X^i$.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RoundCoeffs<F: Field>(pub Vec<F>);

impl<F: Field> RoundCoeffs<F> {
	/// Truncate one coefficient from the polynomial to a more compact round proof.
	pub fn truncate(mut self) -> RoundProof<F> {
		let new_len = self.0.len().saturating_sub(1);
		self.0.truncate(new_len);
		RoundProof(self)
	}
}

impl<F: Field> Add<&Self> for RoundCoeffs<F> {
	type Output = RoundCoeffs<F>;

	fn add(mut self, rhs: &Self) -> Self::Output {
		self += rhs;
		self
	}
}

impl<F: Field> AddAssign<&Self> for RoundCoeffs<F> {
	fn add_assign(&mut self, rhs: &Self) {
		if self.0.len() < rhs.0.len() {
			self.0.resize(rhs.0.len(), F::ZERO);
		}

		for (lhs_i, &rhs_i) in self.0.iter_mut().zip(rhs.0.iter()) {
			*lhs_i += rhs_i;
		}
	}
}

impl<F: Field> Mul<F> for RoundCoeffs<F> {
	type Output = RoundCoeffs<F>;

	fn mul(mut self, rhs: F) -> Self::Output {
		self *= rhs;
		self
	}
}

impl<F: Field> MulAssign<F> for RoundCoeffs<F> {
	fn mul_assign(&mut self, rhs: F) {
		for coeff in self.0.iter_mut() {
			*coeff *= rhs;
		}
	}
}

/// A sumcheck round proof is a univariate polynomial in monomial basis with the coefficient of the
/// highest-degree term truncated off.
///
/// Since the verifier knows the claimed sum of the polynomial values at the points 0 and 1, the
/// high-degree term coefficient can be easily recovered. Truncating the coefficient off saves a
/// small amount of proof data.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct RoundProof<F: Field>(RoundCoeffs<F>);

impl<F: Field> RoundProof<F> {
	/// Recovers all univariate polynomial coefficients from the compressed round proof.
	///
	/// The prover has sent coefficients for the purported ith round polynomial
	/// * $r_i(X) = \sum_{j=0}^d a_j * X^j$
	/// However, the prover has not sent the highest degree coefficient $a_d$.
	/// The verifier will need to recover this missing coefficient.
	///
	/// Let $s$ denote the current round's claimed sum.
	/// The verifier expects the round polynomial $r_i$ to satisfy the identity
	/// * $s = r_i(0) + r_i(1)$
	/// Using
	///     $r_i(0) = a_0$
	///     $r_i(1) = \sum_{j=0}^d a_j$
	/// There is a unique $a_d$ that allows $r_i$ to satisfy the above identity.
	/// Specifically
	///     $a_d = s - a_0 - \sum_{j=0}^{d-1} a_j$
	///
	/// Not sending the whole round polynomial is an optimization.
	/// In the unoptimized version of the protocol, the verifier will halt and reject
	/// if given a round polynomial that does not satisfy the above identity.
	pub fn recover(self, sum: F) -> RoundCoeffs<F> {
		let RoundProof(RoundCoeffs(mut coeffs)) = self;
		let first_coeff = coeffs.first().copied().unwrap_or(F::ZERO);
		let last_coeff = sum - first_coeff - coeffs.iter().sum::<F>();
		coeffs.push(last_coeff);
		RoundCoeffs(coeffs)
	}

	/// The truncated polynomial coefficients.
	pub fn coeffs(&self) -> &[F] {
		&self.0 .0
	}
}

/// A sumcheck batch proof.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Proof<F: Field> {
	/// The round proofs for each round.
	pub rounds: Vec<RoundProof<F>>,
	/// The claimed evaluations of all multilinears at the point defined by the sumcheck verifier
	/// challenges.
	///
	/// The structure is a vector of vectors of field elements. Each entry of the outer vector
	/// corresponds to one [`SumcheckClaim`] in a batch. Each inner vector contains the evaluations
	/// of the multilinears referenced by that claim.
	pub multilinear_evals: Vec<Vec<F>>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct BatchSumcheckOutput<F: Field> {
	pub challenges: Vec<F>,
	pub multilinear_evals: Vec<Vec<F>>,
}
