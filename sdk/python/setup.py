from setuptools import setup, find_packages

setup(
    name="hyperswitch-prism",
    version="0.0.4",
    description="Hyperswitch Payments SDK — Python client for connector integrations via UniFFI FFI",
    packages=find_packages(where="src"),
    package_dir={"": "src"},
    package_data={
        "payments": ["generated/*.dylib", "generated/*.so", "generated/*.py"],
    },
    include_package_data=True,
    python_requires=">=3.9",
    install_requires=[
        "httpx[http2]>=0.27.0",
        "protobuf>=6.31.1",
    ],
)
