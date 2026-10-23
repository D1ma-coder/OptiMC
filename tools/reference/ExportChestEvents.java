// CPU-only 26.2 block-event, lid controller, model transform and part references.
import com.google.gson.*;import java.nio.file.*;import java.util.*;
import net.minecraft.core.*;import net.minecraft.core.registries.*;import net.minecraft.world.level.block.*;import net.minecraft.world.level.block.entity.*;import net.minecraft.network.*;import net.minecraft.network.protocol.game.*;
import net.minecraft.client.model.object.chest.ChestModel;import net.minecraft.client.renderer.blockentity.ChestRenderer;import net.minecraft.world.level.block.state.properties.ChestType;
public class ExportChestEvents {
 public static void main(String[] args)throws Exception {
  net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();var result=new JsonObject();var packets=new JsonArray();
  for(var block:List.of(Blocks.CHEST,Blocks.TRAPPED_CHEST,Blocks.ENDER_CHEST,Blocks.COPPER_CHEST.weathering().unaffected()))for(int viewers:new int[]{0,1,255}){
   var buffer=new RegistryFriendlyByteBuf(io.netty.buffer.Unpooled.buffer(),RegistryAccess.fromRegistryOfRegistries(BuiltInRegistries.REGISTRY));var packet=new ClientboundBlockEventPacket(new BlockPos(-31,-22,47),block,1,viewers);ClientboundBlockEventPacket.STREAM_CODEC.encode(buffer,packet);byte[] body=new byte[buffer.readableBytes()];buffer.readBytes(body);buffer.release();var row=new JsonObject();row.addProperty("block",BuiltInRegistries.BLOCK.getId(block));row.addProperty("viewers",viewers);row.addProperty("body",Base64.getEncoder().encodeToString(body));packets.add(row);
  }result.add("packets",packets);
  var controller=new ChestLidController();var ticks=new JsonArray();
  for(int tick=0;tick<36;tick++){
   boolean open=tick<4 || tick>=7 && tick<22;float initial=controller.getOpenness(1);controller.shouldBeOpen(open);controller.tickLid();var row=new JsonObject();row.addProperty("open",open);row.addProperty("initial",initial);var samples=new JsonArray();for(float partial:new float[]{0,.25f,.5f,.75f,1})samples.add(controller.getOpenness(partial));row.add("samples",samples);ticks.add(row);
  }result.add("ticks",ticks);
  var transforms=new JsonArray();
  for(var direction:Direction.Plane.HORIZONTAL){var row=new JsonObject();row.addProperty("facing",direction.getName());float[] values=new float[16];ChestRenderer.modelTransformation(direction).getMatrix().get(values);row.add("matrix",new Gson().toJsonTree(values));var connected=new JsonObject();for(var type:List.of(ChestType.LEFT,ChestType.RIGHT)){var state=Blocks.CHEST.defaultBlockState().setValue(ChestBlock.FACING,direction).setValue(ChestBlock.TYPE,type);var d=ChestBlock.getConnectedDirection(state);connected.add(type.getSerializedName(),new Gson().toJsonTree(new int[]{d.getStepX(),d.getStepY(),d.getStepZ()}));}row.add("connected",connected);transforms.add(row);
  }result.add("transforms",transforms);
  var poses=new JsonArray();for(String half:List.of("single","left","right"))for(int step=0;step<=10;step++){
   var root=(half.equals("single")?ChestModel.createSingleBodyLayer():half.equals("left")?ChestModel.createDoubleBodyLeftLayer():ChestModel.createDoubleBodyRightLayer()).bakeRoot();var model=new ChestModel(root);float open=step/10f;float f=1-open;model.setupAnim(1-f*f*f);var row=new JsonObject();row.addProperty("half",half);row.addProperty("open",open);row.add("root",ExportSigns.part(root));poses.add(row);
  }result.add("poses",poses);
  Files.writeString(Path.of(args[0]),new Gson().toJson(result));
 }
}
