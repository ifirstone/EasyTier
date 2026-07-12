export interface ReleaseAsset {
    name: string;
    browser_download_url: string;
    size: number;
}

export interface Release {
    tag_name: string;
    prerelease: boolean;
    assets: ReleaseAsset[];
}

const PROXY_PREFIX = 'https://hub.04510451.xyz';

export function proxyDownloadUrl(url: string): string {
    if (url.startsWith('https://github.com/')) {
        return PROXY_PREFIX + '/' + url;
    }
    return url;
}

export async function fetchLatestRelease(): Promise<Release | null> {
    const res = await fetch('https://api.github.com/repos/EasyTier/EasyTier/releases/latest');
    if (!res.ok) {
        return null;
    }
    return (await res.json()) as Release;
}

export async function fetchReleases(): Promise<Release[]> {
    const res = await fetch('https://api.github.com/repos/EasyTier/EasyTier/releases?per_page=20');
    if (!res.ok) {
        return [];
    }
    return (await res.json()) as Release[];
}

export function getArchKey(): string {
    if (typeof navigator === 'undefined') {
        return 'unknown';
    }
    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes('win')) {
        if (ua.includes('x86_64') || ua.includes('win64')) return 'windows-x86_64';
        if (ua.includes('wow64')) return 'windows-x86_64';
        return 'windows-x86_64';
    }
    if (ua.includes('mac')) {
        if (ua.includes('arm') || ua.includes('aarch64')) return 'macos-aarch64';
        return 'macos-x86_64';
    }
    if (ua.includes('linux')) {
        if (ua.includes('aarch64') || ua.includes('arm64')) return 'linux-aarch64';
        if (ua.includes('x86_64')) return 'linux-x86_64';
        if (ua.includes('i686')) return 'linux-i686';
        if (ua.includes('armv7')) return 'linux-armv7';
        if (ua.includes('arm')) return 'linux-arm';
    }
    return 'unknown';
}

export function matchCoreAsset(release: Release, archKey: string): ReleaseAsset | null {
    const normalized = archKey.toLowerCase();
    const candidates = release.assets.filter((asset) => {
        const name = asset.name.toLowerCase();
        if (name.includes('gui')) return false;
        if (normalized === 'unknown') return name.includes('core') || name.includes('easytier');
        return name.includes(normalized) || name.includes(archKey.split('-')[0]);
    });
    if (candidates.length === 0) return null;
    return candidates[0];
}
