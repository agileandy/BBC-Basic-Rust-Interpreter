//! Matrix and machine learning functions (issue #190). These are extensions
//! of this interpreter. BBC BASIC has no form for them.
//!
//! - A matrix is an array with 2 dimensions. The element (row, column) is at
//!   the index `a(row, column)`, and row 0 and column 0 are in use.
//! - A vector is an array with 1 dimension.
//! - The values are in storage order (row by row), as `crate::arrays` has
//!   them. The dimensions are the sizes, not the top indices.
//!
//! This module has no access to the executor. The algorithms use no
//! dependency: Gauss-Jordan elimination with a partial pivot gives the
//! inverse and the determinant.

use crate::error::{BBCBasicError, Result};

/// A pivot with an absolute value below this limit makes a matrix singular
pub const SINGULAR_LIMIT: f64 = 1e-12;

/// A statement with a target array and a source array
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixStatement {
    Transpose,
    Inverse,
    Relu,
    Sigmoid,
    Tanh,
    Softmax,
}

/// A function that gives a number from 1 or 2 arrays
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatrixFunction {
    Det,
    Mean,
    Stdev,
    Min,
    Max,
    Dot,
}

/// The values and the dimensions of a numeric array
#[derive(Debug, Clone, PartialEq)]
pub struct Numbers {
    pub values: Vec<f64>,
    pub dimensions: Vec<usize>,
}

impl MatrixStatement {
    /// The statement with this name, such as `MATTRANSPOSE`
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "MATTRANSPOSE" => Some(Self::Transpose),
            "MATINVERSE" => Some(Self::Inverse),
            "MATRELU" => Some(Self::Relu),
            "MATSIGMOID" => Some(Self::Sigmoid),
            "MATTANH" => Some(Self::Tanh),
            "MATSOFTMAX" => Some(Self::Softmax),
            _ => None,
        }
    }

    /// The values of the result for the source array. The result has the
    /// same storage order as the source. A transpose of an R by C matrix
    /// gives a C by R matrix.
    pub fn apply(self, source: &Numbers) -> Result<Vec<f64>> {
        let values = &source.values;
        match self {
            Self::Transpose => {
                let (rows, columns) = shape(source)?;
                Ok((0..columns * rows)
                    .map(|index| values[(index % rows) * columns + index / rows])
                    .collect())
            }
            Self::Inverse => gauss_jordan(values, square_size(source)?)
                .1
                .ok_or(BBCBasicError::SingularMatrix),
            Self::Relu => Ok(values.iter().map(|x| x.max(0.0)).collect()),
            Self::Sigmoid => Ok(values.iter().map(|x| 1.0 / (1.0 + (-x).exp())).collect()),
            Self::Tanh => Ok(values.iter().map(|x| x.tanh()).collect()),
            Self::Softmax => {
                // A vector is one row. Each row of a matrix is separate.
                let row_length = source.dimensions.last().copied().unwrap_or(values.len());
                Ok(values.chunks(row_length.max(1)).flat_map(softmax).collect())
            }
        }
    }
}

impl MatrixFunction {
    /// The function with this name, such as `MATDET`
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "MATDET" => Some(Self::Det),
            "MATMEAN" => Some(Self::Mean),
            "MATSTDEV" => Some(Self::Stdev),
            "MATMIN" => Some(Self::Min),
            "MATMAX" => Some(Self::Max),
            "MATDOT" => Some(Self::Dot),
            _ => None,
        }
    }

    /// The number of array arguments
    pub fn argument_count(self) -> usize {
        match self {
            Self::Dot => 2,
            _ => 1,
        }
    }

    /// The value of the function for the arrays. The caller gives
    /// `argument_count` arrays.
    pub fn evaluate(self, arrays: &[Numbers]) -> Result<f64> {
        let first = &arrays[0];
        let values = &first.values;
        let count = values.len() as f64;
        match self {
            Self::Det => Ok(gauss_jordan(values, square_size(first)?).0),
            Self::Mean => Ok(values.iter().sum::<f64>() / count),
            Self::Stdev => {
                // The population form: divide by the number of elements
                let mean = values.iter().sum::<f64>() / count;
                let squares: f64 = values.iter().map(|x| (x - mean) * (x - mean)).sum();
                Ok((squares / count).sqrt())
            }
            Self::Min => Ok(values.iter().copied().fold(f64::INFINITY, f64::min)),
            Self::Max => Ok(values.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
            Self::Dot => {
                let second = &arrays[1];
                if first.dimensions.len() != 1
                    || second.dimensions.len() != 1
                    || values.len() != second.values.len()
                {
                    return Err(BBCBasicError::TypeMismatch);
                }
                Ok(values.iter().zip(&second.values).map(|(a, b)| a * b).sum())
            }
        }
    }
}

/// The rows and the columns of a matrix. An array that does not have 2
/// dimensions gives Type mismatch.
fn shape(array: &Numbers) -> Result<(usize, usize)> {
    match array.dimensions[..] {
        [rows, columns] => Ok((rows, columns)),
        _ => Err(BBCBasicError::TypeMismatch),
    }
}

/// The size of a square matrix. Other arrays give Type mismatch.
fn square_size(array: &Numbers) -> Result<usize> {
    match shape(array)? {
        (rows, columns) if rows == columns => Ok(rows),
        _ => Err(BBCBasicError::TypeMismatch),
    }
}

/// Gauss-Jordan elimination with a partial pivot of an N by N matrix. Give
/// the determinant and the inverse. A singular matrix has the determinant 0
/// and no inverse.
fn gauss_jordan(values: &[f64], size: usize) -> (f64, Option<Vec<f64>>) {
    let mut left = values.to_vec();
    let mut right = vec![0.0; size * size];
    for index in 0..size {
        right[index * size + index] = 1.0;
    }
    let mut determinant = 1.0;

    for column in 0..size {
        let pivot_row = (column..size)
            .max_by(|&a, &b| {
                let a = left[a * size + column].abs();
                let b = left[b * size + column].abs();
                a.total_cmp(&b)
            })
            .expect("the range is not empty");
        let pivot = left[pivot_row * size + column];
        if pivot.abs() < SINGULAR_LIMIT {
            return (0.0, None);
        }
        if pivot_row != column {
            for k in 0..size {
                left.swap(pivot_row * size + k, column * size + k);
                right.swap(pivot_row * size + k, column * size + k);
            }
            determinant = -determinant;
        }
        determinant *= pivot;

        for k in 0..size {
            left[column * size + k] /= pivot;
            right[column * size + k] /= pivot;
        }
        for row in (0..size).filter(|&row| row != column) {
            let factor = left[row * size + column];
            if factor != 0.0 {
                for k in 0..size {
                    left[row * size + k] -= factor * left[column * size + k];
                    right[row * size + k] -= factor * right[column * size + k];
                }
            }
        }
    }
    (determinant, Some(right))
}

/// The softmax of one row. The maximum is subtracted before the exponent, so
/// that large values do not give an infinite result.
fn softmax(row: &[f64]) -> Vec<f64> {
    let max = row.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let exponents: Vec<f64> = row.iter().map(|x| (x - max).exp()).collect();
    let sum: f64 = exponents.iter().sum();
    exponents.into_iter().map(|e| e / sum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ERROR_LIMIT: f64 = 1e-9;

    fn matrix(rows: usize, columns: usize, values: &[f64]) -> Numbers {
        Numbers {
            values: values.to_vec(),
            dimensions: vec![rows, columns],
        }
    }

    fn vector(values: &[f64]) -> Numbers {
        Numbers {
            values: values.to_vec(),
            dimensions: vec![values.len()],
        }
    }

    fn assert_near(actual: &[f64], expected: &[f64]) {
        assert_eq!(actual.len(), expected.len(), "{actual:?}");
        for (a, e) in actual.iter().zip(expected) {
            assert!((a - e).abs() < ERROR_LIMIT, "{actual:?} != {expected:?}");
        }
    }

    #[test]
    fn test_the_names_are_known() {
        assert_eq!(
            MatrixStatement::from_name("MATTRANSPOSE"),
            Some(MatrixStatement::Transpose)
        );
        assert_eq!(
            MatrixStatement::from_name("MATSOFTMAX"),
            Some(MatrixStatement::Softmax)
        );
        assert_eq!(MatrixStatement::from_name("MATDET"), None);
        assert_eq!(
            MatrixFunction::from_name("MATDOT"),
            Some(MatrixFunction::Dot)
        );
        assert_eq!(
            MatrixFunction::from_name("MATSTDEV"),
            Some(MatrixFunction::Stdev)
        );
        assert_eq!(MatrixFunction::from_name("MATRELU"), None);
        assert_eq!(MatrixFunction::Dot.argument_count(), 2);
        assert_eq!(MatrixFunction::Det.argument_count(), 1);
    }

    #[test]
    fn test_transpose_a_2_by_3_matrix() {
        let source = matrix(2, 3, &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        let result = MatrixStatement::Transpose.apply(&source).unwrap();
        assert_eq!(result, vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
    }

    #[test]
    fn test_transpose_a_vector_is_type_mismatch() {
        let result = MatrixStatement::Transpose.apply(&vector(&[1.0, 2.0]));
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
    }

    #[test]
    fn test_inverse_of_a_2_by_2_matrix() {
        let source = matrix(2, 2, &[4.0, 7.0, 2.0, 6.0]);
        let result = MatrixStatement::Inverse.apply(&source).unwrap();
        assert_near(&result, &[0.6, -0.7, -0.2, 0.4]);
    }

    #[test]
    fn test_inverse_that_needs_a_row_swap() {
        let source = matrix(3, 3, &[0.0, 1.0, 2.0, 1.0, 0.0, 3.0, 4.0, -3.0, 8.0]);
        let inverse = MatrixStatement::Inverse.apply(&source).unwrap();
        // The source times the inverse is the identity
        let mut product = vec![0.0; 9];
        for row in 0..3 {
            for column in 0..3 {
                product[row * 3 + column] = (0..3)
                    .map(|k| source.values[row * 3 + k] * inverse[k * 3 + column])
                    .sum();
            }
        }
        assert_near(&product, &[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
    }

    #[test]
    fn test_inverse_of_a_singular_matrix_is_an_error() {
        let source = matrix(2, 2, &[1.0, 2.0, 2.0, 4.0]);
        let result = MatrixStatement::Inverse.apply(&source);
        assert_eq!(result, Err(BBCBasicError::SingularMatrix));
    }

    #[test]
    fn test_inverse_of_a_matrix_that_is_not_square_is_type_mismatch() {
        let source = matrix(2, 3, &[1.0; 6]);
        let result = MatrixStatement::Inverse.apply(&source);
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
    }

    #[test]
    fn test_determinant() {
        let det = |source: Numbers| MatrixFunction::Det.evaluate(&[source]).unwrap();
        assert_near(&[det(matrix(1, 1, &[5.0]))], &[5.0]);
        assert_near(&[det(matrix(2, 2, &[4.0, 7.0, 2.0, 6.0]))], &[10.0]);
        // A row swap changes the sign
        assert_near(&[det(matrix(2, 2, &[0.0, 1.0, 1.0, 0.0]))], &[-1.0]);
        let source = matrix(3, 3, &[2.0, -3.0, 1.0, 2.0, 0.0, -1.0, 1.0, 4.0, 5.0]);
        assert_near(&[det(source)], &[49.0]);
        // A singular matrix has the determinant 0
        assert_near(&[det(matrix(2, 2, &[1.0, 2.0, 2.0, 4.0]))], &[0.0]);
    }

    #[test]
    fn test_determinant_of_a_matrix_that_is_not_square_is_type_mismatch() {
        let result = MatrixFunction::Det.evaluate(&[matrix(1, 2, &[1.0, 2.0])]);
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
        let result = MatrixFunction::Det.evaluate(&[vector(&[1.0])]);
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
    }

    #[test]
    fn test_statistics_of_all_elements() {
        let source = matrix(2, 4, &[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]);
        let value =
            |function: MatrixFunction| function.evaluate(std::slice::from_ref(&source)).unwrap();
        assert_near(&[value(MatrixFunction::Mean)], &[5.0]);
        // The population standard deviation
        assert_near(&[value(MatrixFunction::Stdev)], &[2.0]);
        assert_near(&[value(MatrixFunction::Min)], &[2.0]);
        assert_near(&[value(MatrixFunction::Max)], &[9.0]);
    }

    #[test]
    fn test_dot_product_of_2_vectors() {
        let a = vector(&[1.0, 2.0, 3.0]);
        let b = vector(&[4.0, -5.0, 6.0]);
        assert_eq!(MatrixFunction::Dot.evaluate(&[a, b]), Ok(12.0));
    }

    #[test]
    fn test_dot_product_of_different_sizes_or_a_matrix_is_type_mismatch() {
        let result = MatrixFunction::Dot.evaluate(&[vector(&[1.0, 2.0]), vector(&[1.0])]);
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
        let result =
            MatrixFunction::Dot.evaluate(&[matrix(1, 2, &[1.0, 2.0]), matrix(1, 2, &[1.0, 2.0])]);
        assert_eq!(result, Err(BBCBasicError::TypeMismatch));
    }

    #[test]
    fn test_activation_functions() {
        let source = vector(&[-2.0, 0.0, 3.0]);
        let relu = MatrixStatement::Relu.apply(&source).unwrap();
        assert_eq!(relu, vec![0.0, 0.0, 3.0]);
        let sigmoid = MatrixStatement::Sigmoid.apply(&source).unwrap();
        let expected: Vec<f64> = [-2.0f64, 0.0, 3.0]
            .iter()
            .map(|x| 1.0 / (1.0 + (-x).exp()))
            .collect();
        assert_near(&sigmoid, &expected);
        assert_near(&sigmoid[1..2], &[0.5]);
        let tanh = MatrixStatement::Tanh.apply(&source).unwrap();
        assert_near(&tanh, &[(-2.0f64).tanh(), 0.0, 3.0f64.tanh()]);
    }

    #[test]
    fn test_softmax_of_a_vector_is_over_all_elements() {
        let result = MatrixStatement::Softmax
            .apply(&vector(&[1.0, 2.0, 3.0]))
            .unwrap();
        let sum: f64 = [1.0f64, 2.0, 3.0].iter().map(|x| x.exp()).sum();
        assert_near(
            &result,
            &[1.0f64.exp() / sum, 2.0f64.exp() / sum, 3.0f64.exp() / sum],
        );
    }

    #[test]
    fn test_softmax_of_a_matrix_is_over_each_row() {
        let source = matrix(2, 2, &[1.0, 1.0, 0.0, 1000.0]);
        let result = MatrixStatement::Softmax.apply(&source).unwrap();
        assert_near(&result, &[0.5, 0.5, 0.0, 1.0]);
    }

    #[test]
    fn test_softmax_of_large_values_is_finite() {
        let result = MatrixStatement::Softmax
            .apply(&vector(&[1000.0, 1000.0]))
            .unwrap();
        assert_near(&result, &[0.5, 0.5]);
    }
}
