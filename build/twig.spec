# This is the spec file for building the twig RPM package.

# --- Metadata ---
Name:           twig
Version:        0.2.5
Release:        1%{?dist}
Summary:        The core API service is deployed on the guest server and runs via the Canopy Panel and rfs ecosystem interface.

License:        AGPL-3.0
URL:            https://github.com/rfshub/twig
Source0:        %{name}-%{version}.tar.gz

# --- Build Dependencies ---
BuildRequires:  rust-packaging
BuildRequires:  cargo
BuildRequires:  systemd-devel

# --- Runtime Dependencies ---
Requires:       iproute
Requires:       sysstat
Requires:       util-linux
Requires:       kernel-tools
Requires:       fastfetch

%description
The core API service is deployed on the guest server and runs via the Canopy Panel and rfs ecosystem interface.

# --- Build Preparation ---
%prep
%setup -q

# --- Build Step ---
%build
# Build the application using cargo
%cargo_build --release

# --- Installation Step ---
%install
# Install the binary into the correct location
%cargo_install

# --- Files Included in the Package ---
%files
%license LICENSE
%{_bindir}/%{name}

# --- Changelog ---
%changelog
* Sat Aug 02 2025 Canmi (Canmi21) <canmicn@gmail.com> - 0.2.5-1
- Initial RPM release
