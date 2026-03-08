import urllib.request
import json
import os
import re
import time

BASE_URL = "https://physics.nist.gov/PhysRefData/XrayMassCoef/"
ELEMENT_TAB_URL = BASE_URL + "tab3.html"
COMPOSITION_TAB_URL = BASE_URL + "tab2.html"


def fetch_url(url):
    req = urllib.request.Request(
        url,
        data=None,
        headers={
            "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/91.0.4472.114 Safari/537.36"
        },
    )
    with urllib.request.urlopen(req) as response:
        return response.read().decode("utf-8")


def fetch_element_data():
    print("Fetching element links...")
    html = fetch_url(ELEMENT_TAB_URL)

    element_links = re.findall(r'href="(ElemTab/z\d+\.html)"', html)
    element_links = list(set(element_links))
    element_links.sort()

    elements_data = {}

    for link in element_links:
        z_match = re.search(r"z(\d+)\.html", link)
        if not z_match:
            continue
        z = int(z_match.group(1))

        url = BASE_URL + link
        print(f"Fetching data for Z={z}...")
        try:
            element_html = fetch_url(url)
        except Exception as e:
            print(f"Failed to fetch Z={z}: {e}")
            continue

        name_match = re.search(
            r"coefficients\s*-\s*(.*?)</title>", element_html, re.IGNORECASE
        )
        element_name = name_match.group(1).strip() if name_match else f"Element_{z}"

        pre_match = re.search(
            r"<pre>(.*?)</pre>", element_html, re.DOTALL | re.IGNORECASE
        )
        if not pre_match:
            continue

        lines = pre_match.group(1).splitlines()
        energy_list = []
        mu_over_rho_list = []

        for line in lines:
            line = line.strip()
            if not line:
                continue
            parts = line.split()
            if len(parts) >= 2:
                try:
                    energy = float(parts[0])
                    mu_over_rho = float(parts[1])
                    energy_list.append(energy)
                    mu_over_rho_list.append(mu_over_rho)
                except ValueError:
                    continue

        # Zip, sort, and unzip to ensure energies are in ascending order
        combined = sorted(zip(energy_list, mu_over_rho_list))
        sorted_energies = [x[0] for x in combined]
        sorted_mu = [x[1] for x in combined]

        elements_data[z] = {
            "name": element_name,
            "energies": sorted_energies,
            "mu_over_rho": sorted_mu,
        }
        time.sleep(0.05)

    return elements_data


def fetch_composition_data():
    print("Fetching composition data from tab2.html...")
    html = fetch_url(COMPOSITION_TAB_URL)

    # Each material is in a <TR VALIGN="top">
    # We want TD 1 (Name), TD 4 (Density), TD 5 (Composition)
    # Using regex to find all TRs. It's crude but might work if the HTML is consistent.
    tr_pattern = re.compile(r'<TR VALIGN="top">(.*?)</TR>', re.DOTALL | re.IGNORECASE)
    td_pattern = re.compile(r"<TD.*?>(.*?)</TD>", re.DOTALL | re.IGNORECASE)

    compositions_data = {}

    for tr_content in tr_pattern.findall(html):
        tds = td_pattern.findall(tr_content)
        if len(tds) >= 5:
            name = re.sub(r"<.*?>", "", tds[0]).strip()
            density_str = re.sub(r"<.*?>", "", tds[3]).strip()
            comp_raw = tds[4]  # Has <BR>

            try:
                density = float(density_str)
            except ValueError:
                density = 0.0

            composition = {}
            # Format: "1: 0.102000<BR>"
            comp_clean = re.sub(r"[\s\n\r]+", " ", comp_raw)
            # Find all "Z: Fraction" matches
            matches = re.findall(r"(\d+):\s*([0-9\.]+)", comp_clean)
            for z_str, frac_str in matches:
                composition[int(z_str)] = float(frac_str)

            if composition:
                compositions_data[name] = {
                    "density": density,
                    "composition": composition,
                }
                print(f"Parsed composition for {name}")

    return compositions_data


def main():
    if not os.path.exists("data"):
        os.makedirs("data")

    elements = fetch_element_data()
    if elements:
        with open("data/elements.json", "w") as f:
            json.dump(elements, f, indent=2)
        print("Saved data/elements.json")

    compositions = fetch_composition_data()
    if compositions:
        with open("data/compositions.json", "w") as f:
            json.dump(compositions, f, indent=2)
        print("Saved data/compositions.json")


if __name__ == "__main__":
    main()
