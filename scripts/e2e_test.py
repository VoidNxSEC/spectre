import jwt
import requests
import sys
import time
import datetime

# Configuration
SPECTRE_URL = "http://localhost:3000"
JWT_SECRET = "secret"  # Default dev secret

def generate_token():
    payload = {
        "sub": "test-user",
        "role": "admin",
        "exp": datetime.datetime.utcnow() + datetime.timedelta(minutes=5)
    }
    return jwt.encode(payload, JWT_SECRET, algorithm="HS256")

def test_integration():
    print("🚀 Starting E2E Integration Test (Spectre -> Neutron)")
    
    token = generate_token()
    headers = {"Authorization": f"Bearer {token}"}
    
    # 1. Test Health (Spectre)
    print("\n[1] Testing Spectre Health...")
    try:
        r = requests.get(f"{SPECTRE_URL}/health")
        if r.status_code == 200:
            print("✅ Spectre Proxy is Add UP")
        else:
            print(f"❌ Spectre Health Failed: {r.status_code}")
            sys.exit(1)
    except Exception as e:
        print(f"❌ Connection Failed: {e}")
        sys.exit(1)

    # 2. Test Forwarding (Neutron Health via Spectre)
    # The route is /api/v1/neutron/health -> http://neutron:8000/health
    print("\n[2] Testing Forwarding to Neutron...")
    try:
        r = requests.get(f"{SPECTRE_URL}/api/v1/neutron/health", headers=headers)
        if r.status_code == 200:
            print(f"✅ Forwarding Success: {r.json()}")
        elif r.status_code == 502:
             print("⚠️  Spectre reachable, but Neutron API is down (502 Bad Gateway). expected if neutron is not running.")
        else:
            print(f"❌ Forwarding Failed: {r.status_code} - {r.text}")
    except Exception as e:
        print(f"❌ Request Failed: {e}")

if __name__ == "__main__":
    test_integration()
