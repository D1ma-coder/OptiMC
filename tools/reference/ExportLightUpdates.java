// CPU-only official packet and DataLayer nibble reference, no light propagation/GPU.
import com.google.gson.*;import java.nio.file.*;import java.util.*;
import net.minecraft.network.*;import net.minecraft.network.protocol.game.*;import net.minecraft.world.level.chunk.DataLayer;
public class ExportLightUpdates {
 static class WeatherLevel extends net.minecraft.client.multiplayer.ClientLevel {
  private WeatherLevel(){super(null,null,null,null,0,0,null,false,0L,0);}
 }

 static BitSet bits(int... ids){var b=new BitSet();for(int id:ids)b.set(id);return b;}
 static JsonObject row(String label,int[] sky,int[] block,int[] emptySky,int[] emptyBlock,List<byte[]> skies,List<byte[]> blocks){
  var b=new FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());b.writeVarInt(-1);b.writeVarInt(-2);
  b.writeBitSet(bits(sky));b.writeBitSet(bits(block));b.writeBitSet(bits(emptySky));b.writeBitSet(bits(emptyBlock));
  b.writeVarInt(skies.size());for(var a:skies)b.writeByteArray(a);b.writeVarInt(blocks.size());for(var a:blocks)b.writeByteArray(a);
  var packet=ClientboundLightUpdatePacket.STREAM_CODEC.decode(b);if(b.isReadable())throw new AssertionError();b.release();
  var out=new FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());ClientboundLightUpdatePacket.STREAM_CODEC.encode(out,packet);byte[] body=new byte[out.readableBytes()];out.readBytes(body);out.release();
  var r=new JsonObject();r.addProperty("label",label);r.addProperty("id",0x30);r.addProperty("body",Base64.getEncoder().encodeToString(body));
  return r;
 }
 public static void main(String[] args)throws Exception {
  net.minecraft.SharedConstants.tryDetectVersion();net.minecraft.server.Bootstrap.bootStrap();
  var pattern=new DataLayer();var inverse=new DataLayer();var expected=new JsonArray();
  for(int y=0;y<16;y++)for(int z=0;z<16;z++)for(int x=0;x<16;x++){int n=(x+2*y+3*z)&15;pattern.set(x,y,z,n);inverse.set(x,y,z,15-n);}
  for(int y=0;y<16;y++)for(int z=0;z<16;z++)for(int x=0;x<16;x++)expected.add(pattern.get(x,y,z));
  var root=new JsonObject();root.add("nibbles",expected);var rows=new JsonArray();
  rows.add(row("initial",new int[]{0,1,25,70},new int[]{1,2},new int[]{1,2,3},new int[]{0,3},List.of(pattern.getData(),new DataLayer(15).getData(),new DataLayer(7).getData(),new DataLayer(10).getData()),List.of(inverse.getData(),new DataLayer(4).getData())));
  rows.add(row("partial",new int[]{},new int[]{2},new int[]{1},new int[]{},List.of(),List.of(new DataLayer(9).getData())));
  var uf=sun.misc.Unsafe.class.getDeclaredField("theUnsafe");uf.setAccessible(true);var unsafe=(sun.misc.Unsafe)uf.get(null);
  var level=(WeatherLevel)unsafe.allocateInstance(WeatherLevel.class);var weather=new JsonArray();
  var types=new ClientboundGameEventPacket.Type[]{ClientboundGameEventPacket.START_RAINING,ClientboundGameEventPacket.STOP_RAINING,ClientboundGameEventPacket.RAIN_LEVEL_CHANGE,ClientboundGameEventPacket.THUNDER_LEVEL_CHANGE};
  var rain=net.minecraft.world.level.Level.class.getDeclaredField("rainLevel");rain.setAccessible(true);var thunder=net.minecraft.world.level.Level.class.getDeclaredField("thunderLevel");thunder.setAccessible(true);
  for(int type=0;type<4;type++)for(float v:new float[]{-1,-0.0f,0,.25f,1,2,Float.NaN}) {
   var b=new FriendlyByteBuf(io.netty.buffer.Unpooled.buffer());ClientboundGameEventPacket.STREAM_CODEC.encode(b,new ClientboundGameEventPacket(types[type],v));var body=new byte[b.readableBytes()];b.readBytes(body);b.release();
   if(type==0)level.setRainLevel(0);if(type==1)level.setRainLevel(1);if(type==2)level.setRainLevel(v);if(type==3)level.setThunderLevel(v);
   var row=new JsonObject();row.addProperty("body",Base64.getEncoder().encodeToString(body));row.addProperty("rainBits",Float.floatToRawIntBits(rain.getFloat(level)));row.addProperty("thunderBits",Float.floatToRawIntBits(thunder.getFloat(level)));weather.add(row);
  }root.add("weather",weather);
  root.add("packets",rows);Files.writeString(Path.of(args[0]),new Gson().toJson(root));
 }
}
