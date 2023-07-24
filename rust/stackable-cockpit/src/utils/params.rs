use std::{
    collections::HashMap,
    fmt::{Display, Formatter},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use snafu::{ResultExt, Snafu};

#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// Parameter descibes a common parameter format. This format is used in demo and stack definitions
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct Parameter {
    /// Parameter description
    pub description: String,

    /// Parameter default value
    pub default: String,

    /// Parameer value
    #[serde(skip)]
    pub value: String,

    /// Parameter name
    pub name: String,
}

#[derive(Debug, Snafu, PartialEq)]
pub enum IntoParametersError {
    #[snafu(display("raw parameter parse error"))]
    ParseError { source: RawParameterParseError },

    #[snafu(display("invalid parameter '{parameter}', expected one of {expected}"))]
    InvalidParameter { parameter: String, expected: String },
}

pub trait IntoParameters: Sized + IntoRawParameters {
    fn into_params<T>(
        self,
        valid_parameters: T,
    ) -> Result<HashMap<String, String>, IntoParametersError>
    where
        T: AsRef<[Parameter]>,
    {
        let raw_parameters = self.into_raw_params().context(ParseSnafu)?;
        let parameters = valid_parameters.as_ref();

        let mut parameters: HashMap<String, String> = parameters
            .iter()
            .map(|p| (p.name.clone(), p.default.clone()))
            .collect();

        for raw_paramater in raw_parameters {
            if !parameters.contains_key(&raw_paramater.name) {
                return Err(IntoParametersError::InvalidParameter {
                    parameter: raw_paramater.name,
                    expected: valid_parameters
                        .as_ref()
                        .iter()
                        .map(|p| p.name.clone())
                        .collect::<Vec<String>>()
                        .join(", "),
                });
            }
            parameters.insert(raw_paramater.name, raw_paramater.value);
        }

        Ok(parameters)
    }
}

impl IntoParameters for Vec<String> {}
impl IntoParameters for &String {}
impl IntoParameters for String {}
impl IntoParameters for &str {}

/// RawParameter describes a common raw parameter format. Raw parameters are passed in as strings and have the following
/// format: `<NAME>=<VALUE>`.
#[derive(Debug, PartialEq)]
pub struct RawParameter {
    /// Parameter value
    pub value: String,

    /// Parameter name
    pub name: String,
}

#[derive(Debug, Snafu, PartialEq)]
pub enum RawParameterParseError {
    #[snafu(display("invalid equal sign count in parameter, expected one"))]
    InvalidEqualSignCount,

    #[snafu(display("invalid parameter value, cannot be empty"))]
    InvalidParameterValue,

    #[snafu(display("invalid parameter name, cannot be empty"))]
    InvalidParameterName,

    #[snafu(display("invalid (empty) parameter input"))]
    InvalidParameterInput,
}

impl Display for RawParameter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}={}", self.name, self.value)
    }
}

impl FromStr for RawParameter {
    type Err = RawParameterParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let input = s.trim();

        // Empty input is not allowed
        if input.is_empty() {
            return Err(RawParameterParseError::InvalidParameterInput);
        }

        // Split at each equal sign
        let parts: Vec<&str> = input.split('=').collect();
        let len = parts.len();

        // If there are more than 2 equal signs, return error
        // because of invalid spec format
        if len > 2 {
            return Err(RawParameterParseError::InvalidEqualSignCount);
        }

        // Only specifying a key is not valid
        if len == 1 {
            return Err(RawParameterParseError::InvalidParameterValue);
        }

        // If there is an equal sign, but no key before
        if parts[0].is_empty() {
            return Err(RawParameterParseError::InvalidParameterName);
        }

        // If there is an equal sign, but no value after
        if parts[1].is_empty() {
            return Err(RawParameterParseError::InvalidParameterValue);
        }

        Ok(Self {
            name: parts[0].to_string(),
            value: parts[1].to_string(),
        })
    }
}

impl TryFrom<String> for RawParameter {
    type Error = RawParameterParseError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(value.as_str())
    }
}

impl TryFrom<&str> for RawParameter {
    type Error = RawParameterParseError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

pub trait IntoRawParameters: Sized {
    fn into_raw_params(self) -> Result<Vec<RawParameter>, RawParameterParseError>;
}

impl IntoRawParameters for &String {
    fn into_raw_params(self) -> Result<Vec<RawParameter>, RawParameterParseError> {
        self.to_owned().into_raw_params()
    }
}

impl IntoRawParameters for String {
    fn into_raw_params(self) -> Result<Vec<RawParameter>, RawParameterParseError> {
        self.as_str().into_raw_params()
    }
}

impl IntoRawParameters for &str {
    fn into_raw_params(self) -> Result<Vec<RawParameter>, RawParameterParseError> {
        let input = self.trim();

        if input.is_empty() {
            return Err(RawParameterParseError::InvalidParameterInput);
        }

        let mut params = Vec::new();

        let parts: Vec<&str> = input.split(' ').collect();
        for part in parts {
            let param: RawParameter = part.parse()?;
            params.push(param);
        }

        Ok(params)
    }
}

impl IntoRawParameters for Vec<String> {
    fn into_raw_params(self) -> Result<Vec<RawParameter>, RawParameterParseError> {
        let parameters = self
            .iter()
            .map(|s| s.parse())
            .collect::<Result<Vec<RawParameter>, RawParameterParseError>>()?;

        Ok(parameters)
    }
}

#[cfg(test)]
mod test {
    use crate::utils::params::{
        IntoParameters, IntoParametersError, IntoRawParameters, Parameter, RawParameter,
        RawParameterParseError,
    };

    #[test]
    fn single_parameter_str() {
        match RawParameter::try_from("param=value") {
            Ok(param) => {
                assert_eq!(param.name, "param".to_string());
                assert_eq!(param.value, "value".to_string());
            }
            Err(err) => panic!("{err}"),
        }
    }

    #[test]
    fn single_parameter_string() {
        match RawParameter::try_from("param=value".to_string()) {
            Ok(param) => {
                assert_eq!(param.name, "param".to_string());
                assert_eq!(param.value, "value".to_string());
            }
            Err(err) => panic!("{err}"),
        }
    }

    #[test]
    fn single_parameter_no_value() {
        match RawParameter::try_from("param") {
            Ok(param) => panic!("SHOULD FAIL: {param}"),
            Err(err) => assert_eq!(err, RawParameterParseError::InvalidParameterValue),
        }
    }

    #[test]
    fn single_parameter_equal_sign_no_value() {
        match RawParameter::try_from("param=") {
            Ok(param) => panic!("SHOULD FAIL: {param}"),
            Err(err) => assert_eq!(err, RawParameterParseError::InvalidParameterValue),
        }
    }

    #[test]
    fn single_parameter_only_equal_sign() {
        match RawParameter::try_from("=") {
            Ok(param) => panic!("SHOULD FAIL: {param}"),
            Err(err) => assert_eq!(err, RawParameterParseError::InvalidParameterName),
        }
    }

    #[test]
    fn single_parameter_multi_equal_sign() {
        match RawParameter::try_from("param=value=invalid") {
            Ok(param) => panic!("SHOULD FAIL: {param}"),
            Err(err) => assert_eq!(err, RawParameterParseError::InvalidEqualSignCount),
        }
    }

    #[test]
    fn single_parameter_multi_only_equal_sign() {
        match RawParameter::try_from("==") {
            Ok(param) => panic!("SHOULD FAIL: {param}"),
            Err(err) => assert_eq!(err, RawParameterParseError::InvalidEqualSignCount),
        }
    }

    #[test]
    fn multi_raw_parameters_str() {
        match "param1=value1 param2=value2".into_raw_params() {
            Ok(params) => {
                assert_eq!(params.len(), 2);
                let mut iter = params.iter();

                let p = iter.next();
                assert!(p.is_some());
                assert_eq!(
                    p.unwrap(),
                    &RawParameter {
                        name: "param1".into(),
                        value: "value1".into()
                    }
                );

                let p = iter.next();
                assert!(p.is_some());
                assert_eq!(
                    p.unwrap(),
                    &RawParameter {
                        name: "param2".into(),
                        value: "value2".into()
                    }
                );

                let p = iter.next();
                assert!(p.is_none());
            }
            Err(err) => panic!("{err}"),
        }
    }

    #[test]
    fn multi_raw_parameters_string() {
        match "param1=value1 param2=value2".to_string().into_raw_params() {
            Ok(params) => {
                assert_eq!(params.len(), 2);
                let mut iter = params.iter();

                let p = iter.next();
                assert!(p.is_some());
                assert_eq!(
                    p.unwrap(),
                    &RawParameter {
                        name: "param1".into(),
                        value: "value1".into()
                    }
                );

                let p = iter.next();
                assert!(p.is_some());
                assert_eq!(
                    p.unwrap(),
                    &RawParameter {
                        name: "param2".into(),
                        value: "value2".into()
                    }
                );

                let p = iter.next();
                assert!(p.is_none());
            }
            Err(err) => panic!("{err}"),
        }
    }

    #[test]
    fn multi_parameter_valid() {
        let valid_parameters = vec![Parameter {
            description: "Description1".into(),
            default: "Default value 1".into(),
            name: "param1".into(),
            value: "".into(),
        }];

        let input = "param1=value1";

        match input.into_params(valid_parameters) {
            Ok(validated) => {
                assert_eq!(validated.len(), 1);

                if let Some(value) = validated.get(&"param1".to_string()) {
                    assert_eq!(*value, "value1".to_string());
                    return;
                }

                panic!("No parameter in map with name param1");
            }
            Err(err) => panic!("{err}"),
        }
    }

    #[test]
    fn multi_parameter_invalid() {
        let valid_parameters = vec![Parameter {
            description: "Description1".into(),
            default: "Default value 1".into(),
            name: "param1".into(),
            value: "".into(),
        }];

        let input = "param2=value2";

        match input.into_params(valid_parameters) {
            Ok(validated) => panic!("SHOULD FAIL: {validated:?}"),
            Err(err) => assert_eq!(
                err,
                IntoParametersError::InvalidParameter {
                    parameter: "param2".into(),
                    expected: "param1".into()
                }
            ),
        }
    }
}
