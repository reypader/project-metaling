use super::*;

fn valid_request() -> CreateCharacterRequest {
    CreateCharacterRequest {
        account_id: AccountId(2000000),
        name: "ValidChar".to_string(),
        slot: 0,
        hair_color: 0,
        hair_style: 1,
        starting_job: 0,
        sex: Sex::Male,
    }
}

#[test]
fn test_validate_create_character_request_valid() {
    let req = valid_request();
    assert_eq!(validate_create_character_request(&req, 9), Ok(()));

    let mut doram_req = valid_request();
    doram_req.starting_job = 4218;
    doram_req.sex = Sex::Female;
    assert_eq!(validate_create_character_request(&doram_req, 9), Ok(()));
}

#[test]
fn test_validate_create_character_request_slot_out_of_bounds() {
    let mut req = valid_request();
    req.slot = 9;
    assert_eq!(
        validate_create_character_request(&req, 9),
        Err(CreateCharacterError::IneligibleSlot)
    );
}

#[test]
fn test_validate_create_character_request_invalid_sex() {
    let mut req = valid_request();
    req.sex = Sex::Server;
    assert_eq!(
        validate_create_character_request(&req, 9),
        Err(CreateCharacterError::CreationDenied)
    );
}

#[test]
fn test_validate_create_character_request_invalid_job() {
    let mut req = valid_request();
    req.starting_job = 1;
    assert_eq!(
        validate_create_character_request(&req, 9),
        Err(CreateCharacterError::CreationDenied)
    );
}

#[test]
fn test_validate_create_character_request_hair_bounds() {
    let mut req_hair_style = valid_request();
    req_hair_style.hair_style = i16::MAX as u16 + 1;
    assert_eq!(
        validate_create_character_request(&req_hair_style, 9),
        Err(CreateCharacterError::CreationDenied)
    );

    let mut req_hair_color = valid_request();
    req_hair_color.hair_color = i16::MAX as u16 + 1;
    assert_eq!(
        validate_create_character_request(&req_hair_color, 9),
        Err(CreateCharacterError::CreationDenied)
    );
}

#[test]
fn test_validate_create_character_request_name_length() {
    let mut short_req = valid_request();
    short_req.name = "abc".to_string();
    assert_eq!(
        validate_create_character_request(&short_req, 9),
        Err(CreateCharacterError::CreationDenied)
    );

    let mut long_req = valid_request();
    long_req.name = "abcdefghijklmnopqrstuvwx".to_string();
    assert_eq!(
        validate_create_character_request(&long_req, 9),
        Err(CreateCharacterError::CreationDenied)
    );
}

#[test]
fn test_validate_create_character_request_name_spaces() {
    let mut lead_space = valid_request();
    lead_space.name = " Name".to_string();
    assert_eq!(
        validate_create_character_request(&lead_space, 9),
        Err(CreateCharacterError::InvalidNameSymbols)
    );

    let mut trail_space = valid_request();
    trail_space.name = "Name ".to_string();
    assert_eq!(
        validate_create_character_request(&trail_space, 9),
        Err(CreateCharacterError::InvalidNameSymbols)
    );
}

#[test]
fn test_validate_create_character_request_name_symbols() {
    let mut hash_prefix = valid_request();
    hash_prefix.name = "#Name".to_string();
    assert_eq!(
        validate_create_character_request(&hash_prefix, 9),
        Err(CreateCharacterError::InvalidNameSymbols)
    );

    let mut control_char = valid_request();
    control_char.name = "Bad\u{0001}Name".to_string();
    assert_eq!(
        validate_create_character_request(&control_char, 9),
        Err(CreateCharacterError::InvalidNameSymbols)
    );
}

#[test]
fn test_validate_create_character_request_reserved_name() {
    let mut reserved = valid_request();
    reserved.name = "sErVeR".to_string();
    assert_eq!(
        validate_create_character_request(&reserved, 9),
        Err(CreateCharacterError::NameAlreadyExists)
    );
}

#[test]
fn test_map_db_create_error_row_not_found() {
    let err = map_db_create_error(SqlxError::RowNotFound);
    assert_eq!(
        err,
        CreateCharacterError::Repository(RepositoryError::NotFound)
    );
}
