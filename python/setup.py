from setuptools import setup, find_packages

setup(
    name="adilang",
    version="1.16.0",
    description="ADILang Standalone Python SDK — AI-to-AI Intermediate Representation (IR) Protocol Engine",
    long_description=open("README.md", encoding="utf-8").read(),
    long_description_content_type="text/markdown",
    author="BAGAS ADI PRATAMA S,Kom. & ADI Team",
    author_email="onejr007@gmail.com",
    url="https://github.com/onejr007/adilang",
    packages=find_packages(),
    classifiers=[
        "Programming Language :: Python :: 3",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
        "Topic :: Scientific/Engineering :: Artificial Intelligence",
    ],
    python_requires=">=3.8",
    entry_points={
        "console_scripts": [
            "adilang-cli=adilang.cli:main",
        ],
    },
)
