use solana_program::program_error::ProgramError;
use thiserror::Error;

#[derive(Error, Debug, Copy, Clone)]
pub enum NftError {
    #[error("Unauthorized")]               Unauthorized,
    #[error("Invalid PDA")]                InvalidPda,
    #[error("Collection is full")]         CollectionFull,
    #[error("Not the owner")]              NotOwner,
    #[error("Wrong NFT kind")]             WrongNftKind,
    #[error("Insufficient funds")]         InsufficientFunds,
    #[error("Nothing to forward")]         NothingToForward,
    #[error("Nothing to claim")]           NothingToClaim,
    #[error("Fee exceeds 50%")]            FeeTooHigh,
    #[error("Fractions exceed 10000 bps")] InvalidFractions,
    #[error("Too many fraction slots")]    TooManyFractions,
    #[error("Fraction accounts mismatch")] FractionAccountMismatch,
    #[error("Wrong proxy target")]         WrongProxyTarget,
    #[error("Wrong fee recipient")]        WrongFeeRecipient,
    #[error("Cannot fraction this NFT")]   CannotFraction,
    #[error("Missing gen0 account")]       MissingGen0Account,
    #[error("Math overflow")]             MathOverflow,
    #[error("Wrong account owner")]        WrongAccountOwner,
    #[error("Wrong program id")]           WrongProgramId,
    #[error("SBT cannot be transferred")]  SbtNotTransferable,
    #[error("Transfer not allowed")]       TransferNotAllowed,
    #[error("Already burned")]             AlreadyBurned,
}

impl From<NftError> for ProgramError {
    fn from(e: NftError) -> Self { ProgramError::Custom(e as u32) }
}