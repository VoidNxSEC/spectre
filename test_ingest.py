import jwt
import time
import json
import urllib.request
import os

secret = "super_secret_test_key_12345!@"
payload = {
    "sub": "integration-test",
    "role": "service",
    "exp": int(time.time()) + 86400
}

token = jwt.encode(payload, secret, algorithm="HS256")
print(f"Generated JWT: {token}")

url = "http://localhost:3000/api/v1/ingest"
headers = {
    "Authorization": f"Bearer {token}",
    "Content-Type": "application/json"
}

data = json.dumps({
    "type": "test.integration.v1",
    "message": "Hello NATS!"
}).encode('utf-8')

req = urllib.request.Request(url, data=data, headers=headers)
try:
    with urllib.request.urlopen(req) as res:
        print(f"Response Status: {res.status}")
        print(f"Response Body: {res.read().decode('utf-8')}")
except urllib.error.HTTPError as e:
    print(f"HTTP Error: {e.code}")
    print(f"Body: {e.read().decode('utf-8')}")
except Exception as e:
    print(f"Error: {e}")
