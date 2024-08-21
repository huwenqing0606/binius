// Copyright 2024 Ulvetanna Inc.

use super::error::Error;
use crate::{
	oracle::{MultilinearOracleSet, MultilinearPolyOracle},
	protocols::gkr_prodcheck::{ProdcheckClaim, ProdcheckWitness},
	witness::{MultilinearExtensionIndex, MultilinearWitness},
};
use binius_field::{
	as_packed_field::{PackScalar, PackedType},
	underlier::UnderlierType,
	Field, PackedField, TowerField,
};
use binius_utils::bail;
use getset::Getters;
use std::iter;

#[derive(Debug, Getters)]
pub struct MsetcheckClaim<F: Field> {
	/// Oracles to the T polynomials
	#[get = "pub"]
	t_oracles: Vec<MultilinearPolyOracle<F>>,
	/// Oracles to the U polynomials
	#[get = "pub"]
	u_oracles: Vec<MultilinearPolyOracle<F>>,
}

impl<F: Field> MsetcheckClaim<F> {
	/// Claim constructor
	pub fn new(
		t_oracles: impl IntoIterator<Item = MultilinearPolyOracle<F>>,
		u_oracles: impl IntoIterator<Item = MultilinearPolyOracle<F>>,
	) -> Result<Self, Error> {
		let t_oracles = t_oracles.into_iter().collect::<Vec<_>>();
		let u_oracles = u_oracles.into_iter().collect::<Vec<_>>();

		relation_sanity_checks(&t_oracles, &u_oracles, |oracle| oracle.n_vars())?;

		Ok(Self {
			t_oracles,
			u_oracles,
		})
	}

	/// Dimensions of the T/U relations.
	pub fn dimensions(&self) -> usize {
		self.t_oracles.len()
	}

	/// Number of variables in each of the multilinear oracles.
	pub fn n_vars(&self) -> usize {
		self.t_oracles.first().expect("non nullary").n_vars()
	}
}

#[derive(Debug, Getters)]
pub struct MsetcheckWitness<'a, PW: PackedField> {
	/// Witnesses to the T polynomials
	#[get = "pub"]
	t_polynomials: Vec<MultilinearWitness<'a, PW>>,
	/// Witnesses to the U polynomials
	#[get = "pub"]
	u_polynomials: Vec<MultilinearWitness<'a, PW>>,
}

impl<'a, PW: PackedField> MsetcheckWitness<'a, PW> {
	/// Witness constructor
	pub fn new(
		t_polynomials: impl IntoIterator<Item = MultilinearWitness<'a, PW>>,
		u_polynomials: impl IntoIterator<Item = MultilinearWitness<'a, PW>>,
	) -> Result<Self, Error> {
		let t_polynomials = t_polynomials.into_iter().collect::<Vec<_>>();
		let u_polynomials = u_polynomials.into_iter().collect::<Vec<_>>();

		relation_sanity_checks(&t_polynomials, &u_polynomials, |witness| witness.n_vars())?;

		Ok(Self {
			t_polynomials,
			u_polynomials,
		})
	}

	/// Dimensions of the T/U relations.
	pub fn dimensions(&self) -> usize {
		self.t_polynomials.len()
	}

	/// Number of variables in each of the witness multilinears.
	pub fn n_vars(&self) -> usize {
		return self.t_polynomials.first().expect("non nullary").n_vars();
	}
}

#[derive(Debug)]
pub struct MsetcheckProveOutput<'a, U: UnderlierType + PackScalar<FW>, F: Field, FW: Field> {
	pub prodcheck_claim: ProdcheckClaim<F>,
	pub prodcheck_witness: ProdcheckWitness<'a, PackedType<U, FW>>,
	pub witness_index: MultilinearExtensionIndex<'a, U, FW>,
}

pub fn reduce_msetcheck_claim<F: TowerField>(
	oracles: &mut MultilinearOracleSet<F>,
	msetcheck_claim: &MsetcheckClaim<F>,
	gamma: F,
	alpha: Option<F>,
) -> Result<ProdcheckClaim<F>, Error> {
	// Claim sanity checks
	let dimensions = msetcheck_claim.dimensions();
	let n_vars = msetcheck_claim.n_vars();

	if alpha.is_some() != (dimensions > 1) {
		bail!(Error::IncorrectAlpha);
	}

	// for a relation represented by the polynomials (T1, .., Tn) and challenges (gamma, alpha),
	// construct a linear combination oracle for gamma + T1 + alpha * T2 + ... + alpha^(n-1) * Tn
	let mut lincom_oracle = |relation_oracles: &[MultilinearPolyOracle<F>]| -> Result<_, Error> {
		let inner_coeffs = iter::successors(Some(F::ONE), |coeff| alpha.map(|alpha| alpha * coeff));
		let inner = inner_coeffs
			.zip(relation_oracles)
			.map(|(coeff, oracle)| (oracle.id(), coeff));
		let oracle_id = oracles.add_linear_combination_with_offset(n_vars, gamma, inner)?;
		Ok(oracles.oracle(oracle_id))
	};

	let t_oracle = lincom_oracle(&msetcheck_claim.t_oracles)?;
	let u_oracle = lincom_oracle(&msetcheck_claim.u_oracles)?;

	let prodcheck_claim = ProdcheckClaim { t_oracle, u_oracle };

	Ok(prodcheck_claim)
}

fn relation_sanity_checks<Column>(
	t: &[Column],
	u: &[Column],
	n_vars: impl Fn(&Column) -> usize,
) -> Result<(), Error> {
	// same dimensionality
	if t.len() != u.len() {
		bail!(Error::IncorrectDimensions);
	}

	// non-nullary
	if t.is_empty() {
		bail!(Error::NullaryRelation);
	}

	// same n_vars
	let first_n_vars = n_vars(t.first().expect("non nullary"));
	let equal_n_vars = t
		.iter()
		.chain(u)
		.all(|column| n_vars(column) == first_n_vars);

	if !equal_n_vars {
		bail!(Error::NumVariablesMismatch);
	}

	Ok(())
}
