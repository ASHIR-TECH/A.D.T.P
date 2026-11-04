"""PyInstaller entry point for the standalone ADTP gateway.

The desktop app ships this frozen binary as `adtp-api` next to
`adtp-desktop`, so a phone can reach the gateway on a machine that has no
Python installation or source checkout.
"""

from adtp_api.main import main

if __name__ == "__main__":
    main()
