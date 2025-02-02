pub trait CrypDec {
    type Input;
    type Output;
    type Error;

    fn encrypt(&self, data: &Self::Input) -> Result<Self::Output, Self::Error>;
    fn decrypt(&self, data: &Self::Input) -> Result<Self::Output, Self::Error>;
}
