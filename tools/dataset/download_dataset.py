import asyncio
import aiohttp
import json
import os
from pathlib import Path
from tqdm.asyncio import tqdm

# --- AYARLAR ---
METADATA_FILE = "./metadata_clean.json"
OUTPUT_DIR = "dataset_downloaded"
CONCURRENT_LIMIT = 20
TIMEOUT = 45

HEADERS = {
    "User-Agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Accept": "image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8"
}

async def download_one(session, item, semaphore):
    url = item.get("source_url")
    file_name = item.get("file_name")
    
    if not url or not url.startswith("http"):
        return False
        
    save_path = Path(OUTPUT_DIR) / file_name
    save_path.parent.mkdir(parents=True, exist_ok=True)
    
    if save_path.exists():
        return True

    async with semaphore:
        for attempt in range(5):
            try:
                async with session.get(url, timeout=TIMEOUT) as response:
                    if response.status == 200:
                        content = await response.read()
                        if len(content) > 1000: 
                            with open(save_path, "wb") as f:
                                f.write(content)
                            return True
                    elif response.status == 403 or response.status == 404:
                        break 
            except Exception:
                await asyncio.sleep(3) 
    return False

async def main():
    if not os.path.exists(METADATA_FILE):
        print(f"❌ Hata: {METADATA_FILE} bulunamadı!")
        return

    with open(METADATA_FILE, "r", encoding="utf-8") as f:
        data = json.load(f)
    
    # URL'si olanları filtrele
    tasks_data = [item for item in data if item.get("source_url")]
    
    print(f"🚀 {len(tasks_data)} görsel indiriliyor... (Concurrent Limit: {CONCURRENT_LIMIT})")
    
    semaphore = asyncio.Semaphore(CONCURRENT_LIMIT)
    
    connector = aiohttp.TCPConnector(limit=CONCURRENT_LIMIT, ssl=False)
    
    async with aiohttp.ClientSession(connector=connector) as session:
        tasks = [download_one(session, item, semaphore) for item in tasks_data]
        
        results = await tqdm.gather(*tasks, desc="İndiriliyor")
        
    success_count = sum(results)
    print(f"\n✅ Tamamlandı! Başarılı: {success_count}/{len(tasks_data)}")
    print(f"   📂 Konum: {os.path.abspath(OUTPUT_DIR)}")

if __name__ == "__main__":
    try:
        asyncio.set_event_loop_policy(asyncio.WindowsSelectorEventLoopPolicy())
    except:
        pass
        
    asyncio.run(main())