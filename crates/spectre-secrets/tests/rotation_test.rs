use chrono::{Duration, Utc};
use spectre_secrets::{
    types::{Secret, SecretId, SecretMetadata},
    RotationEngine, RotationPolicy,
};
use uuid::Uuid;

#[test]
fn test_should_rotate() {
    let engine = RotationEngine::new();
    let id = SecretId(Uuid::new_v4());
    
    // Case 1: Brand new secret (should not rotate)
    let secret = Secret {
        id: id.clone(),
        version: 1,
        algorithm: "aes-gcm".to_string(),
        ciphertext: vec![],
        metadata: SecretMetadata {
            created_at: Utc::now(),
            updated_at: Utc::now(),
            expires_at: None,
            rotation_policy_id: None,
        },
    };

    let policy = RotationPolicy::TimeBased {
        duration: Duration::days(30),
    };

    assert!(!engine.should_rotate(&secret, &policy));

    // Case 2: Old secret (should rotate)
    let old_secret = Secret {
        id: id.clone(),
        version: 1,
        algorithm: "aes-gcm".to_string(),
        ciphertext: vec![],
        metadata: SecretMetadata {
            created_at: Utc::now() - Duration::days(31),
            updated_at: Utc::now() - Duration::days(31),
            expires_at: None,
            rotation_policy_id: None,
        },
    };

    assert!(engine.should_rotate(&old_secret, &policy));
}

#[test]
fn test_rotate_increments_version() {
    let engine = RotationEngine::new();
    let id = SecretId(Uuid::new_v4());
    
    let mut secret = Secret {
        id: id.clone(),
        version: 1,
        algorithm: "aes-gcm".to_string(),
        ciphertext: vec![],
        metadata: SecretMetadata {
            created_at: Utc::now() - Duration::days(31),
            updated_at: Utc::now() - Duration::days(31),
            expires_at: None,
            rotation_policy_id: None,
        },
    };

    let event = engine.rotate(&mut secret).expect("Rotation failed");

    assert_eq!(secret.version, 2);
    // Updated at should be very recent
    assert!(Utc::now() - secret.metadata.updated_at < Duration::seconds(1));
    
    match event {
        spectre_secrets::events::SecretEvent::Rotated { secret_id, old_version, new_version } => {
            assert_eq!(secret_id, id);
            assert_eq!(old_version, 1);
            assert_eq!(new_version, 2);
        }
        _ => panic!("Wrong event type"),
    }
}
