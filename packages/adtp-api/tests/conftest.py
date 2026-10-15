# Keep the import-time app factory satisfied even when pytest is run without
# ADTP_API_TOKEN exported (adtp_api.main calls create_app() at module level).
import os

os.environ.setdefault("ADTP_API_TOKEN", "abcdefghijklmnopqrstuvwxyz0123456789")