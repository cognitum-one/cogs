/* ==============================================================================================================================

    Copyright © 2022 Advanced Architectures

    All rights reserved
    Confidential Information
    Limited Distribution to Authorized Persons Only
    Created and Protected as an Unpublished Work under the U.S.Copyright act of 1976.

    Project Name         : Newport ASIC

    Description          : Fake Tile for debug of Raceway

=================================================================================================================================
    THIS SOFTWARE IS PROVIDED "AS IS" AND WITHOUT ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
    IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE. IN NO EVENT SHALL THE AUTHOR OR CONTRIBUTORS
    BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED
    TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED
    AND ON ANY THEORY OF LIABILITY, WHETHER IN  CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
    ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
=================================================================================================================================
    NOTES:

===============================================================================================================================*/

`ifdef   STANDALONE
`define  tCQ   #1
`endif

module RacewayZero (
   // Output System Interconnect
   output wire          xmiso_clk,        //     97       96     95:88     87:80     79:72     71:64     63:32      31:0
   output wire [97:0]   xmiso,            // 98 {xtmo,    xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}
   input  wire          xmiso_fb,         //     xpop
   // Input System Interconnect
   input  wire          xmosi_clk,        //     97       96     95:88     87:80     79:72     71:64     63:32      31:0
   input  wire [97:0]   xmosi,            // 98 {xresetn, xpush, xcmd[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xwrd[31:0],xadr[31:0]}
   output wire          xmosi_fb,         //     xirdy
   // bmosi/bmiso RAM port
   output wire [33:0]   bmiso,            // 34 {bgnt, back, brdd[31:0]}
   input  wire [66:0]   bmosi,            // 67 {breq, bwr,brd,  bwrd[31:0], badr[31:0]}
   // cmosi/cmiso RAM port
   output wire [33:0]   cmiso,            // 34 {cgnt, cack, crdd[31:0]}
   input  wire [66:0]   cmosi,            // 67 {creq, cwr,crd,  cwrd[31:0], cadr[31:0]}
   // A2S I/O
   output wire [32:0]   imiso,            // 33 {                IOk, IOo[31:0]}
   input  wire [47:0]   imosi,            // 48 {cIO[2:0], iIO[31:0], aIO[12:0]}
   output wire          intpt_fini,       // Raceway access finish
   output wire          intpt_timo,       // Raceway access timeout
   // PUF Control
   input wire  [35:0]   pufvector,
   input wire           puf_enable,
   output wire [ 7:0]   po_ctrl,
   // Globals
   output wire          flag_out,         // To   Tile-0
   input  wire          reset,
   input  wire          clock
   );

`ifdef   RELATIVE_FILENAMES
`include "NEWPORT_IO_addresses.vh"
`else
`include "/proj/TekStart/lokotech/soc/users/romeo/newport_a0/src/include/NEWPORT_IO_addresses.vh"
`endif

// Locals -----------------------------------------------------------------------------------------------------------------------

reg   [95:0]   XMOSI, XMISO;
reg   [31:0]   PMISO;
reg   [ 7:0]   POCTRL;

wire  [65:0]   amiso;
wire  [66:0]   amosi;
wire  [95:0]   pmosi, gmiso, omiso, rmiso, smiso, tmiso;
wire  [31:0]   Return;
wire           pfwr                                ;
wire           RAMack                              ;
wire           for_mem                             ;
wire           swapping                            ;
wire           broadcast0, broadcast1              ;
wire           cdco_ir,    cdco_wr                 ;
wire           gwir, gwor, gwrd, gwwr              ;
wire           pild, pior, pird, poir, posel, powr ;
wire           ld_packet_in, ld_packet_out         ;
wire           xgive, xinit, xmiso_or, xtake       ;

// Pipeline ---------------------------------------------------------------------------------------------------------------------
//                                                               +-+-------+
//                                        =-bmosi-==============>| |       |==============-bmiso-=>
//                                                               | |       |
//                                                               | |       |
//                                        =-cmosi-==============>| | RAM3  |==============-cmiso-=>
//                                                               | |       |                        smiso     tmiso
//                                              +----+           | |       |              rmiso    /         /
//               +++                      ##===>|cast|===amosi==>| |       |==amiso##    /        /         / +++
//               | |     PIPE             ||    +--^-+           +-+-v-v-v-+       ||   /   +-+  / PIPE    /  | |
//               | |    +-----+           |/=======|=====pmosi==>| |=|=|=|==pmiso==oo======>|m| / +-----+ /   | |
//  ==o=xmosi===>| |===>|di do|==pmosi====oo       |             +-+ | | |           ##====>|u|==>|di do|====>| |=====xmiso=>
//    |          +-+    |     |           ||  +----^--------------^--v-v-v--+        || ===>|x|   |     |     +-+
//    |        +---+    |   or|--pior-----||-->                  ld  k b s  >--sel-------\->+-+   |     |     +---+
//    +-xpush->|   |<---|wr rd|<-pird---------<             GLUE            <--poir--||---\-------|ir or|---->|   |---xordy->
//  <-xmosi_fb-|CDC|--->|ir   |           |>-->                             >--powr--------\----->|wr rd|<----|CDC|<--xread--
//             +---+    +-----+           ||  +v-^-v--------------------^-v^+        ||     \     +-----+     +---+
//                                        ||   | | |       GATEWAY      | ||         ||      \
//                                        ||   | | |       +-----+      | ||         ||       omiso = {pmosi[95:64],64'h0}
//                                        ##===|====pmosi=>|di do|======|==|==gmiso==##
//                                        ||   | | |       |     |      | ||
//                                        ||   | | +-gwwr->|wr or|-gwor-+ ||
//                                        ||   | +-<-gwir--|ir rd|<=gwrd--+|
//              ==pufvector===============================>|     |         |
//              ==pufenable---------------||---|---------->|     |         |
//                                        ||   |           |     |         |
//              =-imosi-==================||==============>|     |==============================-imiso-=>
//                                        ||   |           +-----+         |
//                                        ||   |           +-----+         |
//                                        ||   +---------->| PUF |---------+
//                                        ##-------------->| ctrl|------------------------------po_ctrl->
//                                                         +-----+
// Clock Domain Crossing --------------------------------------------------------------------------------------------------------

A2_CDC i_CDCi  (                 //                                                             :        ^ Safe
   // Input Side -- from Raceway //                                              RACEWAY   +----:----+   | Register
   .wCDC    (xmosi[96]     ),    // INPUT  - Write          (FIFO push)            ------->|    :    |   | Load
   .CDCir   (xmosi_fb      ),    // OUTPUT - Input ready    (FIFO not full)        <-------|    :    |   |
   .CDCld   (ld_packet_in  ),    // OUTPUT - Load  input    (Load CDC register             |    :    |---+
   .ireset  (xinit         ),    // INPUT  - reset synchronized to iclock          ------->|    :    |        //JLSW
   .iclock  (xmosi_clk     ),    // INPUT  - clock                                 --------|>   :    |
   // Output Side -- to Pipe     //                                                        |   CDC   |      PIPE
   .rCDC    (xtake         ),    // INPUT  - Read           (FIFO pop)                     |    :    |<------
   .CDCor   (xgive         ),    // OUTPUT - Output Ready   (FIFO not empty)               |    :    |------>
   .oreset  (reset         ),    // INPUT  - Reset synchronized to Tile clock              |    :    |<------
   .oclock  (clock         )     // INPUT  - Tile clock                                    |    :   <|-------
   );                            //                                                        +----:----+
                                 //                                                             :
// Safe Capture of incoming request
always @(posedge xmosi_clk)   if (ld_packet_in) XMOSI[95:0] <= `tCQ xmosi[95:0];

// Reset synchronization --------------------------------------------------------------------------------------------------------

A2_reset u_rst_x (.q(xinit), .reset(~xmosi[97]), .clock(xmosi_clk));   // Synchronised to External clock

// Input Pipe -------------------------------------------------------------------------------------------------------------------

A2_pipe #(96) i_pipe_in (
   // Output Interface
   .pdo     (pmosi[95:0]   ),    //                                                           PIPE
   .por     (pior          ),    //                                                          +-----+
   .prd     (pird          ),    //                                                  ==lane=>|di do|==lane=>
   // Input Interface            //                                                          |     |
   .pdi     (XMOSI[95:0]   ),    //                                                  --give->|wr or|--give->
   .pwr     (xgive         ),    //                                                          |     |
   .pir     (xtake         ),    //                                                  <-take--|ir rd|<-take--
   .reset   (reset         ),    //                                                          +-----+
   .clock   (clock         )     //
   );

// Input decodes ----------------------------------------------------------------------------------------------------------------

assign
   for_rwz     =   pior    & (pmosi[79:72] == 8'h0),                                // Check destination Tile Number == zero
   for_reg     =   for_rwz &  pmosi[95] & (pmosi[31:30] == 2'b11),                  // Register space selected
   for_mem     =   for_rwz &  pmosi[95] & (pmosi[31:30] == 2'b10),                  // Within range or work RAM //JLPL - 11_2_25
   for_gwy     =   for_rwz & ~pmosi[95],                                            // PUF responses also go here!!
   for_puf     =   for_reg & (pmosi[15:00] == ((TILE_base + TILE_PUF_OSCS) << 2));  // Only one register used!

// PUF CONTROL section ----------------------------------------------------------------------------------------------------------

always @(posedge clock) if ( reset ) POCTRL <= `tCQ        8'h0;
                   else if ( pfwr  ) POCTRL <= `tCQ  pmosi[39:32];

assign
   po_ctrl = POCTRL;

// RAM section ------------------------------------------------------------------------------------------------------------------

assign
   amosi    =  {for_mem,     pmosi[92:91],pmosi[63:0]},                          // 67 {areq, awr, ard, awrd[31:0], aadr[31:0]}
   Return   =  {1'b0,pmosi[94:80],pmosi[71:64],pmosi[79:72]};                    // force response, swap src & dst

always @(posedge clock) if (pild) PMISO  <= `tCQ  Return;

RAM_3Pax i_RAM3 (
   .amiso      (amiso      ),
   .bmiso      (bmiso      ),
   .cmiso      (cmiso      ),
   .amosi      (amosi      ),
   .bmosi      (bmosi      ),
   .cmosi      (cmosi      ),
   .clock      (clock      )
   );

assign
   RAMack   =   amiso[64];

// Gateway section --------------------------------------------------------------------------------------------------------------

A2_gatewayTZ_CoP #(GATEWAY_base,GATEWAY_range) i_GW (
   // Request Out
   .miso       (gmiso      ),
   .ordy       (gwor       ),
   .pop        (gwrd       ),
   // Response In
   .mosi       (pmosi      ),
   .push       (gwwr       ),
   .irdy       (gwir       ),
   // Setup
   .imiso      (imiso      ),
   .imosi      (imosi      ),
   .intpt_fini (intpt_fini ),
   .intpt_timo (intpt_timo ),
   .flag       (flag_out   ),
   .myID       (8'h0       ),
   // PUF Oscillator Interface
   .pufvector  (pufvector  ),
   .puf_enable (puf_enable ),
   // Global
   .reset      (reset      ),
   .clock      (clock      )
   );

// Output Pipe ------------------------------------------------------------------------------------------------------------------

assign
   rmiso    =  {PMISO[31:0], amiso[31:0], amiso[63:32]},
   omiso    =  {Return, 64'h0},
   smiso    =   RAMack     ?  rmiso
            :   for_puf    ?  omiso
            :                 gmiso;

A2_pipe #(96) i_pipe_out (
   // Output Interface
   .pdo     (tmiso[95:0]   ),          //                                                          PIPE
   .por     (cdco_wr       ),          //                                                          +-----+
   .prd     (cdco_ir       ),          //                                                  ==lane=>|di do|==lane=>
   // Input Interface                  //                                                          |     |
   .pdi     (smiso[95:0]   ),          //                                                  --give->|wr or|--give->
   .pwr     (powr          ),          //                                                          |     |
   .pir     (poir          ),          //                                                  <-take--|ir rd|<-take--
   .reset   (reset         ),          //                                                          +-----+
   .clock   (clock         )           //
   );

A2_CDC i_CDCo  (                       //                                                             :        ^ Safe
   // Input Side -- from Raceway       //                                              RACEWAY   +----:----+   | Register
   .wCDC    (cdco_wr       ),          // INPUT  - Write          (FIFO push)            ------->|    :    |   | Load
   .CDCir   (cdco_ir       ),          // OUTPUT - Input ready    (FIFO not full)        <-------|    :    |   |
   .CDCld   (ld_packet_out ),          // OUTPUT - Load  input    (Load CDC register             |    :    |---+
   .ireset  (reset         ),          // INPUT  - reset synchronized to iclock          ------->|    :    |
   .iclock  (clock         ),          // INPUT  - clock                                 --------|>   :    |
   // Output Side -- to Pipe           //                                                        |   CDC   |      PIPE
   .rCDC    (xmiso_fb      ),          // INPUT  - Read           (FIFO pop)                     |    :    |<------
   .CDCor   (xmiso_or      ),          // OUTPUT - Output Ready   (FIFO not empty)               |    :    |------>
   .oreset  (xinit         ),          // INPUT  - Reset synchronized to Tile clock              |    :    |<------
   .oclock  (xmiso_clk     )           // INPUT  - Raceway clock                                 |    :   <|-------
   );                                  //                                                        +----:----+
                                       //                                                             :
// Safe Capture of outgoing access
always @(posedge clock) if (ld_packet_out)   XMISO[95:0] <= `tCQ tmiso[95:0];

// Output -----------------------------------------------------------------------------------------------------------------------

assign
   xmiso[97:0] = {1'b0, xmiso_or, XMISO[95:0]}, // 98 {xtmo,xordy, xack[7:0],xtag[7:0],xdst[7:0],xsrc[7:0],xrd0[31:0],xrd1[31:0]}
   xmiso_clk   =   xmosi_clk;

// Glue -------------------------------------------------------------------------------------------------------------------------

// Issues:
// If the current RAM access is a swap or read/clear then the second cycle must block any incoming memory accesses
// An incoming gateway response must wait if the Gaterway is not ready (gwir)
// An incoming PUF access must be able to go straight to the output so must block if the output pipe is full (!pior)
// There are 3 sources of output (RAM,Gateway and PUFCTRL).  A fixed arbiter is sufficient.  Due to the single cycle of the
// RAM is highest priority as it has no pause mechanism.  PUF is next as it must backup the input pipe if the output is not ready
// so Gateway is lowest as it has state machine to handle pauses.

assign
   swapping    =  ~amiso[65],                                                          // RAM is doing a write after read
   broadcast0  =   pmosi[93],                                                          // No Acknowledge first   cycle!
   broadcast1  =   PMISO[29],                                                          // No Acknowledge second cycle!
   gwwr        =   for_gwy,                                                            // Responses go to the Gateway
   pfwr        =   for_puf    & (pmosi[92:91] == 2'b10),                               // Write to PUF ctrl
   pild        =   for_mem    |  for_puf,                                              // Load pipeline for command word feedback
   pird        =   for_mem    ? ~swapping                                              // RAM is not busy -wait!- amosi is highest
               :   for_puf    ?  poir                                                  // priority : needs to go straight through
               :   for_gwy    ?  gwir                                                  // Gateway input is ready
               :   pior,                                                               // Discard if not for tile zero
   powr        =   RAMack     ? ~broadcast1                                            // Kill the output is broadcast operation
               :   for_puf    ? ~broadcast0                                            // Kill the output is broadcast operation
               :                 gwor,                                                 // Gateway gets a look in
   gwrd        =  ~RAMack     & ~for_puf  &  gwor &  poir;                             // Read (pop) data from gateway

endmodule
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
`ifdef   verify_raceway_zero
module   t;

wire          xmiso_clk;
wire [97:0]   xmiso;
wire          xmosi_fb;
wire [33:0]   bmiso;
wire [33:0]   cmiso;
wire [32:0]   imiso;
wire          intpt_fini;
wire          intpt_timo;
wire          flag_out;

reg           xmiso_fb;
reg           xmosi_clk;
reg  [97:0]   xmosi;
reg  [66:0]   bmosi;
reg  [66:0]   cmosi;
reg  [47:0]   imosi;
reg           reset;
reg           clock;

RacewayZero mut (
   .xmiso_clk  (xmiso_clk),
   .xmiso      (xmiso),
   .xmiso_fb   (xmiso_fb),
   .xmosi_clk  (xmosi_clk),
   .xmosi      (xmosi),
   .xmosi_fb   (xmosi_fb),
   .bmiso      (bmiso),
   .bmosi      (bmosi),
   .cmiso      (cmiso),
   .cmosi      (cmosi),
   .imiso      (imiso),
   .imosi      (imosi),
   .intpt_fini (intpt_fini),
   .intpt_timo (intpt_timo),
   .flag_out   (flag_out),
   .reset      (reset),
   .clock      (clock)
   );

initial begin
   xmiso_fb    = 0;
   xmosi_clk   = 0;
   xmosi       = 0;
   bmosi       = 0;
   cmosi       = 0;
   imosi       = 0;
   reset       = 0;
   clock       = 0;
   $stop();
   end

endmodule
`endif
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////
/////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////////




