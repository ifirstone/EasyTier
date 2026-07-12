export function decryptHostnameSuffix(
    hex: string,
    key: string
): { ip: string; mask: string; subnet: string } | null {
    if (!hex || hex.length % 2 !== 0) {
        return null;
    }

    if (!key) {
        return null;
    }

    const keyLen = key.length;
    const parts: number[] = [];

    for (let i = 0; i < hex.length; i += 2) {
        const hexByte = hex.substring(i, i + 2);
        const val = parseInt(hexByte, 16);
        if (Number.isNaN(val)) {
            return null;
        }

        const keyChar = key[i / 2 % keyLen];
        const keyVal = keyChar.charCodeAt(0);
        const original = val ^ keyVal;

        parts.push(original);
    }

    if (parts.length < 5) {
        return null;
    }

    const ip = `${parts[0]}.${parts[1]}.${parts[2]}.${parts[3]}`;
    const mask = String(parts[4]);
    const subnet = `${parts[0]}.${parts[1]}.${parts[2]}.0`;

    return { ip, mask, subnet };
}
