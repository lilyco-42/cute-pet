import native from 'libcute_pet_host.so';

// XComponent 控制器: surface 生命周期回调 → NAPI → Rust ohos backend
class PetXComponentController extends XComponentController {
  onStatus: (msg: string) => void = () => {};
  onSurfaceCreated(surfaceId: string): void {
    this.onStatus(`created id=${surfaceId}`);
    try {
      const ret: number = native.petStart(BigInt(surfaceId), 540, 960);
      this.onStatus(`petStart ret=${ret}`);
    } catch (e) {
      this.onStatus(`petStart ERR ${JSON.stringify(e)}`);
    }
  }

  onSurfaceChanged(surfaceId: string, rect: SurfaceRect): void {
    this.onStatus(`changed ${rect.surfaceWidth}x${rect.surfaceHeight}`);
    try {
      native.petResize(rect.surfaceWidth, rect.surfaceHeight);
    } catch (e) {
      this.onStatus(`petResize ERR ${JSON.stringify(e)}`);
    }
  }

  onSurfaceDestroyed(surfaceId: string): void {
    this.onStatus('destroyed');
    try {
      native.petStop();
    } catch (e) {
      this.onStatus(`petStop ERR ${JSON.stringify(e)}`);
    }
  }
}

@Entry
@Component
struct Index {
  @State status: string = 'init';
  private controller: PetXComponentController = new PetXComponentController();

  aboutToAppear(): void {
    this.controller.onStatus = (msg: string): void => {
      this.status = msg;
      console.info(`CutePetStatus: ${msg}`);
    };
  }

  build() {
    Column() {
      Text(this.status)
        .fontSize(16)
        .fontColor(Color.White)
        .margin(4)
      XComponent({
        type: XComponentType.SURFACE,
        controller: this.controller
      })
        .width('100%')
        .layoutWeight(1)
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Black)
  }
}
