use std::iter;

use ff::Field;

use crate::{
    arithmetic::CurveAffine,
    plonk::{Error, VerifyingKey},
    poly::{
        commitment::{Params, MSM},
        VerifierQuery,
    },
    transcript::{read_n_points, EncodedChallenge, TranscriptRead},
};

use super::super::{ChallengeX, ChallengeY};
use super::Argument;

#[derive(Debug)]
pub struct Committed<C: CurveAffine> {
    pub random_poly_commitment: C,
}

#[derive(Debug)]
pub struct Constructed<C: CurveAffine> {
    h_commitments: Vec<C>,
    random_poly_commitment: C,
}

#[derive(Debug)]
pub struct PartiallyEvaluated<C: CurveAffine> {
    h_commitments: Vec<C>,
    random_poly_commitment: C,
    random_eval: C::Scalar,
}

#[derive(Debug)]
pub struct Evaluated<C: CurveAffine, M: MSM<C>> {
    h_commitment: M,
    random_poly_commitment: C,
    expected_h_eval: C::Scalar,
    random_eval: C::Scalar,
}

impl<C: CurveAffine> Argument<C> {
    pub(in crate::plonk) fn read_commitments_before_y<
        E: EncodedChallenge<C>,
        T: TranscriptRead<C, E>,
    >(
        transcript: &mut T,
    ) -> Result<Committed<C>, Error> {
        let random_poly_commitment = transcript.read_point()?;

        Ok(Committed {
            random_poly_commitment,
        })
    }
}

impl<C: CurveAffine> Committed<C> {
    pub(in crate::plonk) fn read_commitments_after_y<
        E: EncodedChallenge<C>,
        T: TranscriptRead<C, E>,
    >(
        self,
        vk: &VerifyingKey<C>,
        transcript: &mut T,
    ) -> Result<Constructed<C>, Error> {
        // Obtain a commitment to h(X) in the form of multiple pieces of degree n - 1
        let h_commitments = read_n_points(transcript, vk.domain.get_quotient_poly_degree())?;

        Ok(Constructed {
            h_commitments,
            random_poly_commitment: self.random_poly_commitment,
        })
    }
}

impl<C: CurveAffine> Constructed<C> {
    pub(in crate::plonk) fn evaluate_after_x<E: EncodedChallenge<C>, T: TranscriptRead<C, E>>(
        self,
        transcript: &mut T,
    ) -> Result<PartiallyEvaluated<C>, Error> {
        let random_eval = transcript.read_scalar()?;

        Ok(PartiallyEvaluated {
            h_commitments: self.h_commitments,
            random_poly_commitment: self.random_poly_commitment,
            random_eval,
        })
    }
}

impl<C: CurveAffine> PartiallyEvaluated<C> {
    pub(in crate::plonk) fn verify<'params, P: Params<'params, C>>(
        self,
        params: &'params P,
        expressions: impl Iterator<Item = C::Scalar>,
        y: ChallengeY<C>,
        xn: C::Scalar,
    ) -> Evaluated<C, P::MSM> {
        let expected_h_eval = expressions.fold(C::Scalar::ZERO, |h_eval, v| h_eval * &*y + &v);
        let expected_h_eval = expected_h_eval * ((xn - C::Scalar::ONE).invert().unwrap());

        let h_commitment =
            self.h_commitments
                .iter()
                .rev()
                .fold(params.empty_msm(), |mut acc, commitment| {
                    acc.scale(xn);
                    let commitment: C::CurveExt = (*commitment).into();
                    acc.append_term(C::Scalar::ONE, commitment);

                    acc
                });

        Evaluated {
            expected_h_eval,
            h_commitment,
            random_poly_commitment: self.random_poly_commitment,
            random_eval: self.random_eval,
        }
    }
}

impl<C: CurveAffine, M: MSM<C>> Evaluated<C, M> {
    pub(in crate::plonk) fn queries(
        &self,
        x: ChallengeX<C>,
    ) -> impl Iterator<Item = VerifierQuery<C, M>> + Clone {
        iter::empty()
            .chain(Some(VerifierQuery::new_msm(
                &self.h_commitment,
                *x,
                self.expected_h_eval,
            )))
            .chain(Some(VerifierQuery::new_commitment(
                &self.random_poly_commitment,
                *x,
                self.random_eval,
            )))
    }
}
